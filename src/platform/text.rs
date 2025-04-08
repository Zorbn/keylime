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
            let Text { inner, cache } = self;

            unsafe {
                result = inner.get_glyphs(cache, result, text, |inner, cache, glyph, result| {
                    let (glyph_span, glyph_cache_result) = cache.get_glyph_span(inner, glyph);

                    cache.glyph_spans.push(glyph_span);
                    result.worse(glyph_cache_result)
                });
            }
        };

        let spans_end = self.cache.glyph_spans.len();

        let glyph_spans = GlyphSpans {
            spans_start,
            spans_end,
        };

        self.cache.layout_cache.insert(cached_text, glyph_spans);

        (glyph_spans, result)
    }

    pub fn get_glyph_span(&mut self, index: usize) -> GlyphSpan {
        self.cache.glyph_spans[index]
    }

    pub fn swap_caches(&mut self) {
        self.cache.swap_caches();
    }
}
