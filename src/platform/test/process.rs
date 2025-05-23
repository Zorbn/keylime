use std::sync::{Arc, Mutex};

use crate::platform::process::ProcessKind;

use super::result::Result;

pub struct Process {
    pub input: Vec<u8>,
    pub output: Arc<Mutex<Vec<u8>>>,
}

impl Process {
    pub fn new(_commands: &[&str], _kind: ProcessKind) -> Result<Self> {
        Ok(Self {
            input: Vec::new(),
            output: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn flush(&self) {}

    pub fn resize(&self, _width: usize, _height: usize) {}
}
