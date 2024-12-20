use std::{
    ffi::{c_char, CStr, CString},
    path::{Path, PathBuf},
    ptr::null_mut,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use libc::{kevent, EVFILT_VNODE, EV_ADD, EV_CLEAR, EV_DELETE, NOTE_WRITE, O_EVTONLY};
use objc2::rc::Retained;

use super::{
    result::Result,
    view::{View, ViewRef},
};

struct WatchedPath {
    fd: i32,
    path: PathBuf,
    is_in_use: bool,

    // Backing data for the user data pointer passed to kqueue.
    _path_cstr: CString,
}

impl Drop for WatchedPath {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

pub struct FileWatcher {
    kq: i32,

    watched_paths: Vec<WatchedPath>,
    watch_thread_join: Option<JoinHandle<()>>,
    watch_thread_changed_files: Arc<Mutex<Vec<PathBuf>>>,

    changed_files: Vec<PathBuf>,
}

impl FileWatcher {
    pub fn new() -> Self {
        let kq = unsafe { libc::kqueue() };

        Self {
            kq,

            watched_paths: Vec::new(),
            watch_thread_join: None,
            watch_thread_changed_files: Arc::new(Mutex::new(Vec::new())),

            changed_files: Vec::new(),
        }
    }

    pub fn update<'a>(&mut self, files: impl Iterator<Item = &'a Path>) -> Result<()> {
        self.changed_files.clear();

        for watched_path in &mut self.watched_paths {
            watched_path.is_in_use = false;
        }

        'docs: for file in files {
            for watched_path in &mut self.watched_paths {
                if watched_path.path != file {
                    continue;
                }

                watched_path.is_in_use = true;
                continue 'docs;
            }

            let path = CString::new(
                file.as_os_str()
                    .to_str()
                    .ok_or("Failed to convert file path to str")?,
            )
            .map_err(|_| "Failed to convert file path to CString")?;

            let fd = unsafe { libc::open(path.as_ptr(), O_EVTONLY) };

            let add_event = kevent {
                ident: fd as usize,
                filter: EVFILT_VNODE,
                flags: EV_ADD | EV_CLEAR,
                fflags: NOTE_WRITE,
                data: 0,
                udata: path.as_ptr() as _,
            };

            unsafe {
                if libc::kevent(self.kq, &add_event, 1, null_mut(), 0, null_mut()) == -1 {
                    return Err("Failed to add file to kqueue");
                }
            }

            self.watched_paths.push(WatchedPath {
                fd,
                path: PathBuf::from(
                    path.to_str()
                        .map_err(|_| "Failed to convert file path to str")?,
                ),
                is_in_use: true,

                _path_cstr: path,
            });
        }

        for i in (0..self.watched_paths.len()).rev() {
            if !self.watched_paths[i].is_in_use {
                let watched_path = self.watched_paths.remove(i);

                let delete_event = kevent {
                    ident: watched_path.fd as usize,
                    filter: 0,
                    flags: EV_DELETE,
                    fflags: 0,
                    data: 0,
                    udata: null_mut(),
                };

                unsafe {
                    libc::kevent(self.kq, &delete_event, 1, null_mut(), 0, null_mut());
                }
            }
        }

        Ok(())
    }

    pub fn get_changed_files(&mut self) -> &[PathBuf] {
        let mut watch_thread_changed_files = self.watch_thread_changed_files.lock().unwrap();

        self.changed_files
            .extend(watch_thread_changed_files.drain(..));

        &self.changed_files
    }

    pub fn try_start(&mut self, view: &Retained<View>) {
        if self.watch_thread_join.is_some() {
            return;
        }

        self.watch_thread_join = Some(Self::run_watch_thread(
            self.kq,
            self.watch_thread_changed_files.clone(),
            ViewRef::new(view),
        ));
    }

    fn run_watch_thread(
        kq: i32,
        changed_files: Arc<Mutex<Vec<PathBuf>>>,
        view: ViewRef,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            let mut event_list = [kevent {
                ident: 0,
                filter: 0,
                flags: 0,
                fflags: 0,
                data: 0,
                udata: null_mut(),
            }; 32];

            loop {
                let event_count = unsafe {
                    libc::kevent(
                        kq,
                        null_mut(),
                        0,
                        event_list.as_mut_ptr(),
                        event_list.len() as i32,
                        null_mut(),
                    )
                };

                for i in 0..event_count {
                    let event = event_list[i as usize];

                    let path = unsafe { CStr::from_ptr(event.udata as *const c_char) };
                    let path = PathBuf::from(path.to_str().unwrap());

                    let mut changed_files = changed_files.lock().unwrap();
                    changed_files.push(path);
                }

                unsafe {
                    view.update();
                }
            }
        })
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.kq);
        }

        let _ = self.watch_thread_join.take().unwrap().join();
    }
}
