use std::{
    cell::RefCell,
    collections::HashMap,
    hash::{Hash, Hasher},
    mem::swap,
    rc::Rc,
};

use super::platform_impl::{self, text::Glyph};

#[derive(Debug, Default, Clone, Copy)]
pub struct AtlasDimensions {
    pub origin_x: f32,
    pub origin_y: f32,
    pub width: usize,
    pub height: usize,
    pub glyph_width: usize,
    pub glyph_height: usize,
    pub line_height: usize,
}

#[derive(Debug, Default)]
pub struct Atlas {
    pub data: Vec<u8>,
    pub dimensions: AtlasDimensions,
    pub has_color_glyphs: bool,
}

impl Atlas {
    fn copy_to(&self, other: &mut Atlas, offset_x: usize) {
        for y in 0..self.dimensions.height {
            for x in 0..self.dimensions.width {
                let i = (x + y * self.dimensions.width) * 4;

                let other_x = x + offset_x;
                let other_i = (other_x + y * other.dimensions.width) * 4;

                other.data[other_i] = self.data[i];
                other.data[other_i + 1] = self.data[i + 1];
                other.data[other_i + 2] = self.data[i + 2];
                other.data[other_i + 3] = self.data[i + 3];
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphSpans {
    pub spans_start: usize,
    pub spans_end: usize,
}

pub struct CachedLayout {
    pub data: Rc<RefCell<String>>,
    pub start: usize,
    pub end: usize,
}

impl Hash for CachedLayout {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let data = self.data.borrow();

        data[self.start..self.end].hash(state)
    }
}

impl PartialEq for CachedLayout {
    fn eq(&self, other: &Self) -> bool {
        let data = self.data.borrow();
        let other_data = other.data.borrow();

        data[self.start..self.end] == other_data[other.start..other.end]
    }
}

impl Eq for CachedLayout {}

#[derive(Debug, Clone, Copy)]
pub enum GlyphSpan {
    Space,
    Tab,
    Glyph {
        origin_x: f32,
        origin_y: f32,
        x: usize,
        width: usize,
        height: usize,
        advance: usize,
        has_color_glyphs: bool,
    },
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum GlyphCacheResult {
    Hit,
    Miss,
    Resize,
}

impl GlyphCacheResult {
    pub fn worse(self, other: GlyphCacheResult) -> GlyphCacheResult {
        match other {
            GlyphCacheResult::Hit => self,
            GlyphCacheResult::Miss => {
                if self == GlyphCacheResult::Hit {
                    other
                } else {
                    self
                }
            }
            GlyphCacheResult::Resize => other,
        }
    }
}

pub struct TextCache {
    glyph_cache: HashMap<u16, GlyphSpan>,

    pub last_glyph_spans: Vec<GlyphSpan>,
    last_layout_data: Rc<RefCell<String>>,
    pub last_layout_cache: HashMap<CachedLayout, GlyphSpans>,

    pub glyph_spans: Vec<GlyphSpan>,
    pub layout_data: Rc<RefCell<String>>,
    pub layout_cache: HashMap<CachedLayout, GlyphSpans>,

    needs_first_resize: bool,
    atlas_used_width: usize,

    pub atlas: Atlas,
}

impl TextCache {
    pub fn new() -> Self {
        Self {
            glyph_cache: HashMap::new(),

            last_glyph_spans: Vec::new(),
            last_layout_data: Rc::new(RefCell::new(String::new())),
            last_layout_cache: HashMap::new(),

            glyph_spans: Vec::new(),
            layout_data: Rc::new(RefCell::new(String::new())),
            layout_cache: HashMap::new(),

            needs_first_resize: true,
            atlas_used_width: 0,
            atlas: Atlas::default(),
        }
    }

    pub fn glyph_span(
        &mut self,
        text: &mut platform_impl::text::Text,
        glyph: Glyph,
    ) -> (GlyphSpan, GlyphCacheResult) {
        let mut result = if self.needs_first_resize {
            GlyphCacheResult::Resize
        } else {
            GlyphCacheResult::Hit
        };

        self.needs_first_resize = false;

        if let Some(span) = self.glyph_cache.get(&glyph.index) {
            return (*span, result);
        }

        let x = self.atlas_used_width;
        let sub_atlas = unsafe { text.generate_atlas(glyph) }.unwrap();
        let width = sub_atlas.dimensions.width;
        let glyph_right = x + width;

        if glyph_right == 0
            || glyph_right > self.atlas.dimensions.width
            || sub_atlas.dimensions.height > self.atlas.dimensions.height
        {
            let mut new_width = self.atlas.dimensions.width.max(width);

            while glyph_right > new_width {
                new_width *= 2;
            }

            let new_height = self
                .atlas
                .dimensions
                .height
                .max(sub_atlas.dimensions.height);

            let mut new_atlas_dimensions = self.atlas.dimensions;
            new_atlas_dimensions.width = new_width;
            new_atlas_dimensions.height = new_height;

            let mut new_atlas = Atlas {
                data: vec![0u8; new_width * new_height * 4],
                dimensions: new_atlas_dimensions,
                has_color_glyphs: false,
            };

            self.atlas.copy_to(&mut new_atlas, 0);
            self.atlas = new_atlas;

            result = result.worse(GlyphCacheResult::Resize)
        } else {
            result = result.worse(GlyphCacheResult::Miss)
        };

        sub_atlas.copy_to(&mut self.atlas, x);

        let span = GlyphSpan::Glyph {
            origin_x: sub_atlas.dimensions.origin_x,
            origin_y: sub_atlas.dimensions.origin_y,
            x,
            width,
            height: sub_atlas.dimensions.height,
            advance: glyph.advance,
            has_color_glyphs: sub_atlas.has_color_glyphs,
        };

        self.glyph_cache.insert(glyph.index, span);
        self.atlas_used_width += width;

        (span, result)
    }

    pub fn swap_caches(&mut self) {
        swap(&mut self.last_glyph_spans, &mut self.glyph_spans);

        swap(&mut self.last_layout_data, &mut self.layout_data);
        swap(&mut self.last_layout_cache, &mut self.layout_cache);

        self.glyph_spans.clear();

        self.layout_data.borrow_mut().clear();
        self.layout_cache.clear();
    }
}
