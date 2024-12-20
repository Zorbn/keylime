use std::sync::{Arc, Mutex};

use super::{platform_impl, result::Result};

pub struct Pty {
    pub(super) inner: platform_impl::pty::Pty,
}

impl Pty {
    pub fn new(width: isize, height: isize, child_paths: &[&str]) -> Result<Self> {
        let inner = platform_impl::pty::Pty::new(width, height, child_paths)?;

        Ok(Self { inner })
    }

    pub fn flush(&mut self) {
        self.inner.flush();
    }

    pub fn resize(&mut self, width: isize, height: isize) {
        self.inner.resize(width, height);
    }

    pub fn input(&mut self) -> &mut Vec<u32> {
        &mut self.inner.input
    }

    pub fn output(&self) -> &Arc<Mutex<Vec<u32>>> {
        &self.inner.output
    }
}
