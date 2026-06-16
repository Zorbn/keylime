use std::sync::{Condvar, Mutex, MutexGuard};

use super::{platform_impl, result::Result};

#[derive(Debug)]
pub enum ProcessKind {
    Normal,
    Pty { width: usize, height: usize },
}

struct ProcessOutputState {
    buffer: Vec<u8>,
    is_alive: bool,
}

impl ProcessOutputState {
    const MAX_OUTPUT: usize = 1 << 16;

    fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(Self::MAX_OUTPUT),
            is_alive: true,
        }
    }

    fn should_wait(&self) -> bool {
        self.is_alive && self.buffer.len() >= Self::MAX_OUTPUT
    }

    fn enqueue<'a>(&mut self, data: &'a [u8]) -> &'a [u8] {
        if !self.is_alive {
            return &[];
        }

        let allowed = data.len().min(self.buffer.len() - Self::MAX_OUTPUT);
        self.buffer.extend_from_slice(&data[..allowed]);
        &data[allowed..]
    }
}

pub struct DequeuedProcessOutput<'a> {
    state: MutexGuard<'a, ProcessOutputState>,
    condvar: &'a Condvar,
}

impl DequeuedProcessOutput<'_> {
    pub fn data(&mut self) -> &mut Vec<u8> {
        &mut self.state.buffer
    }
}

impl Drop for DequeuedProcessOutput<'_> {
    fn drop(&mut self) {
        self.condvar.notify_all();
        self.state.buffer.clear();
    }
}

pub struct ProcessOutput {
    state: Mutex<ProcessOutputState>,
    condvar: Condvar,
}

impl ProcessOutput {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(ProcessOutputState::new()),
            condvar: Condvar::new(),
        }
    }

    pub fn enqueue<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        let mut state = self.state.lock().unwrap();

        while state.should_wait() {
            state = self.condvar.wait(state).unwrap();
        }

        state.enqueue(data)
    }

    pub fn dequeue(&self) -> DequeuedProcessOutput<'_> {
        DequeuedProcessOutput {
            state: self.state.lock().unwrap(),
            condvar: &self.condvar,
        }
    }

    pub fn kill(&self) {
        let mut state = self.state.lock().unwrap();
        state.is_alive = false;
        self.condvar.notify_all();
    }
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

    pub fn input_output(&mut self) -> (&mut Vec<u8>, DequeuedProcessOutput<'_>) {
        (&mut self.inner.input, self.inner.output.dequeue())
    }
}
