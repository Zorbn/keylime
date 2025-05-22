use std::{
    ffi::CString,
    path::{Path, PathBuf},
    ptr::null_mut,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use libc::{kevent, EVFILT_VNODE, EV_ADD, EV_CLEAR, EV_DELETE, NOTE_DELETE, NOTE_WRITE, O_EVTONLY};
use objc2::rc::Weak;

use crate::pool::{Pooled, PATH_POOL};

use super::{
    result::Result,
    view::{View, ViewRef},
};

struct WatchedPath {
    fd: i32,
    path: Pooled<PathBuf>,
    is_in_use: bool,
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
    changed_fds: Arc<Mutex<Vec<i32>>>,
    deleted_fds: Arc<Mutex<Vec<i32>>>,

    changed_files: Vec<Pooled<PathBuf>>,
}

impl FileWatcher {
    pub fn new() -> Self {
        let kq = unsafe { libc::kqueue() };

        Self {
            kq,

            watched_paths: Vec::new(),
            watch_thread_join: None,
            changed_fds: Arc::new(Mutex::new(Vec::new())),
            deleted_fds: Arc::new(Mutex::new(Vec::new())),

            changed_files: Vec::new(),
        }
    }

    pub fn update<'a>(&mut self, files: impl Iterator<Item = &'a Path>) -> Result<()> {
        self.changed_files.clear();

        self.handle_deleted_fds();
        self.retain_watched_paths(|watched_path| watched_path.is_in_use);

        for watched_path in &mut self.watched_paths {
            watched_path.is_in_use = false;
        }

        self.handle_files(files)?;
        self.retain_watched_paths(|watched_path| watched_path.is_in_use);

        Ok(())
    }

    fn handle_files<'a>(&mut self, files: impl Iterator<Item = &'a Path>) -> Result<()> {
        for file in files {
            if let Some(watched_path) = self
                .watched_paths
                .iter_mut()
                .find(|watched_path| watched_path.path.as_ref() == file)
            {
                watched_path.is_in_use = true;
                continue;
            }

            let c_path = CString::new(
                file.as_os_str()
                    .to_str()
                    .ok_or("Failed to convert file path to str")?,
            )
            .map_err(|_| "Failed to convert file path to CString")?;

            let fd = unsafe { libc::open(c_path.as_ptr(), O_EVTONLY) };

            let add_event = kevent {
                ident: fd as usize,
                filter: EVFILT_VNODE,
                flags: EV_ADD | EV_CLEAR,
                fflags: NOTE_WRITE | NOTE_DELETE,
                data: 0,
                udata: null_mut(),
            };

            unsafe {
                if libc::kevent(self.kq, &add_event, 1, null_mut(), 0, null_mut()) == -1 {
                    return Err("Failed to add file to kqueue");
                }
            }

            let mut path = PATH_POOL.new_item();

            path.push(
                c_path
                    .to_str()
                    .map_err(|_| "Failed to convert file path to str")?,
            );

            self.watched_paths.push(WatchedPath {
                fd,
                path,
                is_in_use: true,
            });
        }

        Ok(())
    }

    fn handle_deleted_fds(&mut self) {
        let mut deleted_fds = self.deleted_fds.lock().unwrap();

        for fd in deleted_fds.drain(..) {
            if let Some(watched_path) = self
                .watched_paths
                .iter_mut()
                .find(|watched_path| watched_path.fd == fd)
            {
                watched_path.is_in_use = false;
            }
        }
    }

    fn retain_watched_paths(&mut self, predicate: fn(&WatchedPath) -> bool) {
        for i in (0..self.watched_paths.len()).rev() {
            if predicate(&self.watched_paths[i]) {
                continue;
            }

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

    pub fn changed_files(&mut self) -> &[Pooled<PathBuf>] {
        let mut changed_fds = self.changed_fds.lock().unwrap();

        for fd in changed_fds.drain(..) {
            if let Some(watched_path) = self
                .watched_paths
                .iter_mut()
                .find(|watched_path| watched_path.fd == fd)
            {
                self.changed_files.push(watched_path.path.clone());
            }
        }

        &self.changed_files
    }

    pub fn try_start(&mut self, view: &Weak<View>) {
        if self.watch_thread_join.is_some() {
            return;
        }

        self.watch_thread_join = Some(Self::run_watch_thread(
            self.kq,
            self.changed_fds.clone(),
            self.deleted_fds.clone(),
            ViewRef::new(view),
        ));
    }

    fn run_watch_thread(
        kq: i32,
        changed_fds: Arc<Mutex<Vec<i32>>>,
        deleted_fds: Arc<Mutex<Vec<i32>>>,
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

                if event_count != 1 {
                    break;
                }

                for i in 0..event_count {
                    let event = event_list[i as usize];

                    if event.fflags & NOTE_DELETE != 0 {
                        let mut deleted_fds = deleted_fds.lock().unwrap();
                        deleted_fds.push(event.ident as i32);

                        continue;
                    }

                    let mut changed_fds = changed_fds.lock().unwrap();
                    changed_fds.push(event.ident as i32);
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

        if let Some(watch_thread_join) = self.watch_thread_join.take() {
            let _ = watch_thread_join.join();
        }
    }
}
