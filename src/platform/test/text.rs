use crate::platform::{
    text::OnGlyph,
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

    pub unsafe fn glyphs(
        &mut self,
        _text_cache: &mut TextCache,
        _glyph_cache_result: GlyphCacheResult,
        _text: &str,
        _on_glyph: OnGlyph,
    ) -> GlyphCacheResult {
        GlyphCacheResult::Hit
    }
}
