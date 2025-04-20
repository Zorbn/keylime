use std::sync::{Arc, Mutex};

use super::result::Result;

pub struct Pty {
    pub input: Vec<u8>,
    pub output: Arc<Mutex<Vec<u8>>>,
}

impl Pty {
    pub fn new(_width: usize, _height: usize, _child_paths: &[&str]) -> Result<Self> {
        Ok(Self {
            input: Vec::new(),
            output: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn flush(&mut self) {}

    pub fn resize(&mut self, _width: usize, _height: usize) {}
}
