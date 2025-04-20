use crate::platform::{
    text::GlyphFn,
    text_cache::{Atlas, GlyphCacheResult, TextCache},
};

use super::result::Result;

#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    pub index: u16,
    pub advance: usize,
}

pub struct Text;

impl Text {
    pub unsafe fn generate_atlas(&mut self, _glyph: Glyph) -> Result<Atlas> {
        Ok(Atlas::default())
    }

    pub unsafe fn get_glyphs(
        &mut self,
        _text_cache: &mut TextCache,
        _glyph_cache_result: GlyphCacheResult,
        _text: &str,
        _glyph_fn: GlyphFn,
    ) -> GlyphCacheResult {
        GlyphCacheResult::Hit
    }
}
