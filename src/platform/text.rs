use super::{platform_impl, result::Result};

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

pub struct Text {
    inner: platform_impl::text::Text,
}

#[cfg(target_os = "windows")]
const BACKUP_FONT_NAME: &str = "Consolas";

#[cfg(target_os = "macos")]
const BACKUP_FONT_NAME: &str = "Menlo";

impl Text {
    pub fn new(font_name: &str, font_size: f32, scale: f32) -> Result<Self> {
        let inner = unsafe {
            platform_impl::text::Text::new(font_name, font_size, scale).or(
                platform_impl::text::Text::new(BACKUP_FONT_NAME, font_size, scale),
            )?
        };

        Ok(Self { inner })
    }

    // For now the atlas is static and only supports ASCII characters.
    // It could be upgraded to support any character and use the atlas
    // as a cache that gets updated when new characters are needed.
    pub fn generate_atlas(&mut self) -> Result<Atlas> {
        unsafe { self.inner.generate_atlas() }
    }
}
