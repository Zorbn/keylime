use std::{
    ffi::CString,
    ops::Deref,
    ptr::{null, null_mut},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use objc2::{rc::Retained, runtime::AnyObject, sel};
use objc2_foundation::{NSNumber, NSObjectNSThreadPerformAdditions};

use crate::text::utf32::{utf32_to_utf8, utf8_to_utf32};

use super::{gfx::KeylimeView, result::Result};

pub struct Pty {
    pub output: Arc<Mutex<Vec<u32>>>,
    pub input: Vec<u32>,
    input_bytes: Vec<u8>,

    read_thread_join: Option<JoinHandle<()>>,

    fd: i32,

    pub view: Arc<Mutex<Option<Retained<KeylimeView>>>>,
    has_view: bool,
}

impl Pty {
    pub fn new(width: isize, height: isize, child_paths: &[&str]) -> Result<Self> {
        let mut window_size = libc::winsize {
            ws_row: height as u16,
            ws_col: width as u16,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let mut fd = 0;
        let pid = unsafe { libc::forkpty(&mut fd, null_mut(), null_mut(), &mut window_size) };

        if pid == 0 {
            let shell = CString::new("zsh").unwrap();
            let args = &[shell.as_ptr(), null()];

            unsafe {
                libc::execvp(shell.as_ptr(), args.as_ptr());
                unreachable!();
            }
        }

        let output = Arc::new(Mutex::new(Vec::new()));
        let view = Arc::new(Mutex::new(None));

        let read_thread_join = Self::run_read_thread(output.clone(), view.clone(), fd);

        Ok(Self {
            output,
            input: Vec::new(),
            input_bytes: Vec::new(),

            read_thread_join: Some(read_thread_join),

            fd,

            view,
            has_view: false,
        })
    }

    pub fn flush(&mut self) {
        if self.input.is_empty() {
            return;
        }

        utf32_to_utf8(&self.input, &mut self.input_bytes);
        self.input.clear();

        unsafe {
            libc::write(
                self.fd,
                self.input_bytes.as_ptr() as _,
                self.input_bytes.len(),
            );
        }

        self.input_bytes.clear();
    }

    pub fn resize(&mut self, width: isize, height: isize) {
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

    pub fn try_set_view(&mut self, view: &Retained<KeylimeView>) {
        if self.has_view {
            return;
        }

        self.has_view = true;

        let mut stored_view = self.view.lock().unwrap();
        *stored_view = Some(view.clone());
    }

    fn run_read_thread(
        output: Arc<Mutex<Vec<u32>>>,
        view: Arc<Mutex<Option<Retained<KeylimeView>>>>,
        fd: i32,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            let mut buffer = [0u8; 1024];

            loop {
                let bytes_read = unsafe { libc::read(fd, buffer.as_mut_ptr() as _, buffer.len()) };

                {
                    let mut output = output.lock().unwrap();
                    utf8_to_utf32(&buffer[..bytes_read as usize], &mut output);
                }

                let view = view.lock().unwrap();

                if let Some(view) = view.as_deref() {
                    unsafe {
                        let arg = NSNumber::new_bool(true);
                        let arg = arg.deref() as *const _ as *const AnyObject;

                        view.performSelectorOnMainThread_withObject_waitUntilDone(
                            sel!(setNeedsDisplay:),
                            Some(&*arg),
                            false,
                        );
                    }
                }
            }
        })
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }

        let _ = self.read_thread_join.take().unwrap().join();
    }
}
