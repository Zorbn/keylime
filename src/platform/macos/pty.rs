use std::{
    ffi::CString,
    ptr::{null, null_mut},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use libc::{kevent, EVFILT_READ, EV_ADD, EV_CLEAR};
use objc2::rc::Retained;

use super::{
    result::Result,
    view::{View, ViewRef},
};

pub struct Pty {
    pub output: Arc<Mutex<Vec<u8>>>,
    pub input: Vec<u8>,

    read_thread_join: Option<JoinHandle<()>>,

    kq: i32,
    fd: i32,
}

impl Pty {
    pub fn new(width: usize, height: usize, child_paths: &[&str]) -> Result<Self> {
        let mut window_size = libc::winsize {
            ws_row: height as u16,
            ws_col: width as u16,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let kq = unsafe { libc::kqueue() };

        let mut fd = 0;
        let pid = unsafe { libc::forkpty(&mut fd, null_mut(), null_mut(), &mut window_size) };

        if pid == 0 {
            for child_path in child_paths {
                let shell = CString::new(*child_path).unwrap();
                let args = &[shell.as_ptr(), null()];

                unsafe {
                    libc::setenv(c"TERM".as_ptr(), c"xterm-256color".as_ptr(), 1);
                    libc::setenv(c"COLORTERM".as_ptr(), c"truecolor".as_ptr(), 1);
                    libc::execvp(shell.as_ptr(), args.as_ptr());
                }
            }

            unsafe {
                libc::exit(1);
            }
        }

        let add_event = kevent {
            ident: fd as usize,
            filter: EVFILT_READ,
            flags: EV_ADD | EV_CLEAR,
            fflags: 0,
            data: 0,
            udata: null_mut(),
        };

        unsafe {
            if libc::kevent(kq, &add_event, 1, null_mut(), 0, null_mut()) == -1 {
                return Err("Failed to add pty to kqueue");
            }
        }

        Ok(Self {
            output: Arc::new(Mutex::new(Vec::new())),
            input: Vec::new(),

            read_thread_join: None,

            kq,
            fd,
        })
    }

    pub fn flush(&mut self) {
        if self.input.is_empty() {
            return;
        }

        unsafe {
            libc::write(self.fd, self.input.as_ptr() as _, self.input.len());
        }

        self.input.clear();
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        let mut window_size = libc::winsize {
            ws_row: height as u16,
            ws_col: width as u16,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        unsafe {
            libc::ioctl(self.fd, libc::TIOCSWINSZ, &mut window_size);
        }
    }

    pub fn try_start(&mut self, view: &Retained<View>) {
        if self.read_thread_join.is_some() {
            return;
        }

        self.read_thread_join = Some(Self::run_read_thread(
            self.output.clone(),
            ViewRef::new(view),
            self.kq,
            self.fd,
        ));
    }

    fn run_read_thread(
        output: Arc<Mutex<Vec<u8>>>,
        view: ViewRef,
        kq: i32,
        fd: i32,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            let mut buffer = [0u8; 1024];

            let mut event_list = [kevent {
                ident: 0,
                filter: 0,
                flags: 0,
                fflags: 0,
                data: 0,
                udata: null_mut(),
            }; 1];

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

                let bytes_read = unsafe { libc::read(fd, buffer.as_mut_ptr() as _, buffer.len()) };

                if !matches!(bytes_read, 0 | -1) {
                    let mut output = output.lock().unwrap();
                    output.extend_from_slice(&buffer[..bytes_read as usize]);
                } else {
                    break;
                }

                unsafe {
                    view.update();
                }
            }
        })
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.kq);
            libc::close(self.fd);
        }

        let _ = self.read_thread_join.take().unwrap().join();
    }
}
