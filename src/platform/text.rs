pub struct Atlas {
    pub data: Vec<u8>,
    pub dimensions: AtlasDimensions,
}

#[derive(Debug)]
pub struct AtlasDimensions {
    pub width: usize,
    pub height: usize,
    pub glyph_offset_x: f32,
    pub glyph_step_x: f32,
    pub glyph_width: f32,
    pub glyph_height: f32,
    pub line_height: f32,
}
