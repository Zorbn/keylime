use crate::text::grapheme::{self, CharCursor};

use super::{
    aliases::PlatformText,
    platform_impl::text::Glyph,
    result::Result,
    text_cache::{CachedLayout, GlyphCacheResult, GlyphSpan, GlyphSpans, TextCache},
};

pub type GlyphFn =
    fn(&mut PlatformText, &mut TextCache, Glyph, GlyphCacheResult) -> GlyphCacheResult;

pub struct Text {
    inner: PlatformText,
    pub cache: TextCache,
}

#[cfg(target_os = "windows")]
const BACKUP_FONT_NAME: &str = "Consolas";

#[cfg(target_os = "macos")]
const BACKUP_FONT_NAME: &str = "Menlo";

impl Text {
    pub fn new(font_name: &str, font_size: f32, scale: f32) -> Result<Self> {
        let inner = unsafe {
            PlatformText::new(font_name, font_size, scale).or(PlatformText::new(
                BACKUP_FONT_NAME,
                font_size,
                scale,
            ))?
        };

        let mut text = Self {
            inner,
            cache: TextCache::new(),
        };

        unsafe {
            let Text {
                ref mut inner,
                ref mut cache,
            } = text;

            inner.get_glyphs(
                cache,
                GlyphCacheResult::Miss,
                "M",
                |inner, cache, glyph, result| {
                    cache.atlas = inner.generate_atlas(glyph).unwrap();
                    result
                },
            );
        }

        Ok(text)
    }

    pub fn get_glyph_spans(&mut self, text: &str) -> (GlyphSpans, GlyphCacheResult) {
        let mut text_cache_data = self.cache.layout_data.borrow_mut();

        let data_start = text_cache_data.len();
        text_cache_data.push_str(text);
        let data_end = text_cache_data.len();

        let cached_text = CachedLayout {
            data: self.cache.layout_data.clone(),
            start: data_start,
            end: data_end,
        };

        drop(text_cache_data);

        if let Some(glyph_spans) = self.cache.layout_cache.get(&cached_text) {
            let mut text_cache_data = self.cache.layout_data.borrow_mut();
            text_cache_data.truncate(data_start);

            return (*glyph_spans, GlyphCacheResult::Hit);
        }

        let mut result = GlyphCacheResult::Hit;
        let spans_start = self.cache.glyph_spans.len();

        if let Some(glyph_spans) = self.cache.last_layout_cache.get(&cached_text) {
            self.cache.glyph_spans.extend_from_slice(
                &self.cache.last_glyph_spans[glyph_spans.spans_start..glyph_spans.spans_end],
            );
        } else {
            result = result.worse(self.get_uncached_layout_glyph_spans(text));
        };

        let spans_end = self.cache.glyph_spans.len();

        let glyph_spans = GlyphSpans {
            spans_start,
            spans_end,
        };

        self.cache.layout_cache.insert(cached_text, glyph_spans);

        (glyph_spans, result)
    }

    fn get_uncached_layout_glyph_spans(&mut self, text: &str) -> GlyphCacheResult {
        let Text { inner, cache } = self;

        let mut glyphs_start = 0;
        let mut char_cursor = CharCursor::new(0, text.len());
        let mut result = GlyphCacheResult::Hit;

        while char_cursor.cur_cursor() < text.len() {
            let mut reset_glyphs_start = true;

            match grapheme::char_at(char_cursor.cur_cursor(), text) {
                " " => {
                    result = result.worse(Self::flush_glyphs(
                        inner,
                        cache,
                        glyphs_start,
                        &char_cursor,
                        text,
                    ));

                    cache.glyph_spans.push(GlyphSpan::Space);
                }
                "\t" => {
                    result = result.worse(Self::flush_glyphs(
                        inner,
                        cache,
                        glyphs_start,
                        &char_cursor,
                        text,
                    ));

                    cache.glyph_spans.push(GlyphSpan::Tab);
                }
                _ => reset_glyphs_start = false,
            }

            char_cursor.next_boundary(text);

            if reset_glyphs_start {
                glyphs_start = char_cursor.cur_cursor();
            }
        }

        result.worse(Self::flush_glyphs(
            inner,
            cache,
            glyphs_start,
            &char_cursor,
            text,
        ))
    }

    fn flush_glyphs(
        inner: &mut PlatformText,
        cache: &mut TextCache,
        glyphs_start: usize,
        char_cursor: &CharCursor,
        text: &str,
    ) -> GlyphCacheResult {
        let glyph_text = &text[glyphs_start..char_cursor.cur_cursor()];

        unsafe {
            inner.get_glyphs(
                cache,
                GlyphCacheResult::Hit,
                glyph_text,
                |inner, cache, glyph, result| {
                    let (glyph_span, glyph_cache_result) = cache.get_glyph_span(inner, glyph);

                    cache.glyph_spans.push(glyph_span);
                    result.worse(glyph_cache_result)
                },
            )
        }
    }

    pub fn get_glyph_span(&mut self, index: usize) -> GlyphSpan {
        self.cache.glyph_spans[index]
    }

    pub fn swap_caches(&mut self) {
        self.cache.swap_caches();
    }
}
