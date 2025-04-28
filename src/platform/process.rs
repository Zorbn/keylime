use std::sync::{Arc, Mutex};

use super::{platform_impl, result::Result};

pub enum ProcessKind {
    Normal,
    Pty { width: usize, height: usize },
}

pub struct Process {
    pub(super) inner: platform_impl::process::Process,
}

impl Process {
    pub fn new(commands: &[&str], kind: ProcessKind) -> Result<Self> {
        let inner = platform_impl::process::Process::new(commands, kind)?;

        Ok(Self { inner })
    }

    pub fn flush(&mut self) {
        self.inner.flush();
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.inner.resize(width, height);
    }

    pub fn input(&mut self) -> &mut Vec<u8> {
        &mut self.inner.input
    }

    pub fn input_output(&mut self) -> (&mut Vec<u8>, &Arc<Mutex<Vec<u8>>>) {
        (&mut self.inner.input, &self.inner.output)
    }
}
