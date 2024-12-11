use std::sync::{Arc, Mutex};

use super::result::Result;

pub struct Pty {
    pub output: Arc<Mutex<Vec<u32>>>,
    pub input: Vec<u32>,
}

impl Pty {
    pub fn new(width: isize, height: isize, child_paths: &[&str]) -> Result<Self> {
        Ok(Self {
            output: Arc::new(Mutex::new(Vec::new())),
            input: Vec::new(),
        })
    }

    pub fn flush(&mut self) {}

    pub fn resize(&mut self, width: isize, height: isize) {}
}
