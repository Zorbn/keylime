use crate::{temp_buffer::TempString, text::line_pool::LinePool};

pub struct EditorBuffers {
    pub lines: LinePool,
    pub text: TempString,
}

impl EditorBuffers {
    pub fn new() -> Self {
        Self {
            lines: LinePool::new(),
            text: TempString::new(),
        }
    }
}
