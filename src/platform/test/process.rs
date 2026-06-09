use std::sync::Arc;

use crate::platform::process::{ProcessKind, ProcessOutput};

use super::result::Result;

pub struct Process {
    pub input: Vec<u8>,
    pub output: Arc<ProcessOutput>,
}

impl Process {
    pub fn new(_commands: &[&str], _kind: ProcessKind) -> Result<Self> {
        Ok(Self {
            input: Vec::new(),
            output: Arc::new(ProcessOutput::new()),
        })
    }

    pub fn flush(&self) {}

    pub fn resize(&self, _width: usize, _height: usize) {}
}
