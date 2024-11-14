#[derive(Clone, Copy)]
pub enum CursorIndex {
    Some(usize),
    Main,
}

impl CursorIndex {
    pub fn unwrap_or(self, default_index: usize) -> usize {
        match self {
            CursorIndex::Some(index) => index,
            CursorIndex::Main => default_index,
        }
    }
}

pub struct CursorIndices {
    i: usize,
    len: usize,
}

impl CursorIndices {
    pub fn new(len: usize) -> Self {
        Self { i: 0, len }
    }
}

impl Iterator for CursorIndices {
    type Item = CursorIndex;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i < self.len {
            let index = CursorIndex::Some(self.i);

            self.i += 1;

            Some(index)
        } else {
            None
        }
    }
}
