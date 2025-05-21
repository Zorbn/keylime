#[derive(Clone, Copy, Debug)]
pub enum CursorIndex {
    Some(usize),
    Main,
}

impl CursorIndex {
    pub fn unwrap_or(self, main_index: usize) -> usize {
        match self {
            Self::Some(index) => index,
            Self::Main => main_index,
        }
    }
}

#[derive(Clone, Copy)]
pub struct CursorIndices {
    i: usize,
    len: usize,
}

impl CursorIndices {
    pub fn new(i: usize, len: usize) -> Self {
        Self { i, len }
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

impl DoubleEndedIterator for CursorIndices {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.len > self.i {
            let index = CursorIndex::Some(self.len - 1);

            self.len -= 1;

            Some(index)
        } else {
            None
        }
    }
}
