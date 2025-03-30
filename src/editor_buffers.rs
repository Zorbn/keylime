use crate::{
    temp_buffer::{TempBuffer, TempString},
    text::{cursor::Cursor, line_pool::LinePool},
};

pub struct EditorBuffers {
    pub lines: LinePool,
    pub cursors: TempBuffer<Cursor>,
    pub text: TempString,
}
