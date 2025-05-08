use std::{
    ffi::CString,
    ptr::{null, null_mut},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use libc::{kevent, EVFILT_READ, EV_ADD, EV_CLEAR};
use objc2::rc::Retained;

use crate::platform::process::ProcessKind;

use super::{
    result::Result,
    view::{View, ViewRef},
};

const PIPE_READ: usize = 0;
const PIPE_WRITE: usize = 1;

pub struct Process {
    pub output: Arc<Mutex<Vec<u8>>>,
    pub input: Vec<u8>,

    read_thread_join: Option<JoinHandle<()>>,

    kq: i32,
    pid: i32,
    read_fd: i32,
    write_fd: i32,
}

impl Process {
    pub fn new(commands: &[&str], kind: ProcessKind) -> Result<Self> {
        let kq = unsafe { libc::kqueue() };
        let mut result_fds = [0, 0];

        if unsafe { libc::pipe(result_fds.as_mut_ptr()) } == -1 {
            return Err("Failed to create result pipe");
        }

        let (read_fd, write_fd, pid) = unsafe {
            match kind {
                ProcessKind::Normal => {
                    let mut stdin_fds = [0, 0];
                    let mut stdout_fds = [0, 0];

                    if libc::pipe(stdin_fds.as_mut_ptr()) == -1
                        || libc::pipe(stdout_fds.as_mut_ptr()) == -1
                    {
                        return Err("Failed to create stdin/stdout pipes");
                    }

                    let pid = libc::fork();

                    if pid <= 0 {
                        if pid == 0 {
                            libc::dup2(stdin_fds[PIPE_READ], libc::STDIN_FILENO);
                            libc::dup2(stdout_fds[PIPE_WRITE], libc::STDOUT_FILENO);
                        }

                        libc::close(stdin_fds[PIPE_READ]);
                        libc::close(stdin_fds[PIPE_WRITE]);
                        libc::close(stdout_fds[PIPE_READ]);
                        libc::close(stdout_fds[PIPE_WRITE]);
                    } else {
                        libc::close(stdin_fds[PIPE_READ]);
                        libc::close(stdout_fds[PIPE_WRITE]);
                    }

                    (stdout_fds[PIPE_READ], stdin_fds[PIPE_WRITE], pid)
                }
                ProcessKind::Pty { width, height } => {
                    let mut window_size = libc::winsize {
                        ws_row: height as u16,
                        ws_col: width as u16,
                        ws_xpixel: 0,
                        ws_ypixel: 0,
                    };

                    let mut fd = 0;
                    let pid = libc::forkpty(&mut fd, null_mut(), null_mut(), &mut window_size);

                    (fd, fd, pid)
                }
            }
        };

        if pid < 0 {
            return Err("Failed to fork process");
        }

        if pid == 0 {
            unsafe {
                libc::close(result_fds[PIPE_READ]);

                let flags = libc::fcntl(result_fds[PIPE_WRITE], libc::F_GETFD) | libc::FD_CLOEXEC;
                libc::fcntl(result_fds[PIPE_WRITE], libc::F_SETFD, flags);
            }

            for command in commands {
                let Some(child_path) = command.split(' ').nth(0) else {
                    continue;
                };

                let child_path = CString::new(child_path).unwrap();

                let args: Vec<CString> = command
                    .split(' ')
                    .map(|arg| CString::new(arg).unwrap())
                    .collect();

                let args: Vec<*const i8> = args
                    .iter()
                    .map(|arg| arg.as_ptr() as _)
                    .chain([null()])
                    .collect();

                unsafe {
                    if matches!(kind, ProcessKind::Pty { .. }) {
                        libc::setenv(c"TERM".as_ptr(), c"xterm-256color".as_ptr(), 1);
                        libc::setenv(c"COLORTERM".as_ptr(), c"truecolor".as_ptr(), 1);
                    }

                    libc::execvp(child_path.as_ptr(), args.as_ptr());
                }
            }

            unsafe {
                let status = 1;

                libc::write(
                    result_fds[PIPE_WRITE],
                    &status as *const _ as _,
                    size_of::<i32>(),
                );

                libc::exit(status);
            }
        }

        let mut child_status = 0;

        unsafe {
            libc::close(result_fds[PIPE_WRITE]);

            libc::read(
                result_fds[PIPE_READ],
                &mut child_status as *mut _ as _,
                size_of::<i32>(),
            );
        }

        if child_status != 0 {
            return Err("Failed to start child");
        }

        let add_event = kevent {
            ident: read_fd as usize,
            filter: EVFILT_READ,
            flags: EV_ADD | EV_CLEAR,
            fflags: 0,
            data: 0,
            udata: null_mut(),
        };

        unsafe {
            if libc::kevent(kq, &add_event, 1, null_mut(), 0, null_mut()) == -1 {
                return Err("Failed to add process to kqueue");
            }
        }

        Ok(Self {
            output: Arc::new(Mutex::new(Vec::new())),
            input: Vec::new(),

            read_thread_join: None,

            kq,
            pid,
            read_fd,
            write_fd,
        })
    }

    pub fn flush(&mut self) {
        if self.input.is_empty() {
            return;
        }

        unsafe {
            libc::write(self.write_fd, self.input.as_ptr() as _, self.input.len());
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
            libc::ioctl(self.write_fd, libc::TIOCSWINSZ, &mut window_size);
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
            self.read_fd,
        ));
    }

    fn run_read_thread(
        output: Arc<Mutex<Vec<u8>>>,
        view: ViewRef,
        kq: i32,
        read_fd: i32,
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

                let bytes_read =
                    unsafe { libc::read(read_fd, buffer.as_mut_ptr() as _, buffer.len()) };

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

impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            libc::kill(self.pid, libc::SIGTERM);
            libc::close(self.kq);
            libc::close(self.read_fd);
            libc::close(self.write_fd);
        }

        if let Some(read_thread_join) = self.read_thread_join.take() {
            let _ = read_thread_join.join();
        }
    }
}
