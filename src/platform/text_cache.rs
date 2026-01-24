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

impl AtlasDimensions {
    pub const ZERO: Self = Self {
        origin_x: 0.0,
        origin_y: 0.0,
        width: 0,
        height: 0,
        glyph_width: 0,
        glyph_height: 0,
        line_height: 0,
    };
}

#[derive(Debug, Default)]
pub struct Atlas {
    pub data: Vec<u8>,
    pub dimensions: AtlasDimensions,
    pub has_color_glyphs: bool,
}

impl Atlas {
    fn copy_to(&self, other: &mut Self, offset_x: usize, offset_y: usize) {
        for y in 0..self.dimensions.height {
            for x in 0..self.dimensions.width {
                let i = (x + y * self.dimensions.width) * 4;

                let other_x = x + offset_x;
                let other_y = y + offset_y;
                let other_i = (other_x + other_y * other.dimensions.width) * 4;

                other.data[other_i] = self.data[i];
                other.data[other_i + 1] = self.data[i + 1];
                other.data[other_i + 2] = self.data[i + 2];
                other.data[other_i + 3] = self.data[i + 3];
            }
        }
    }

    fn ensure_size(&mut self, required_width: usize, required_height: usize) -> bool {
        if required_width <= self.dimensions.width && required_height <= self.dimensions.height {
            return false;
        }

        let new_width = Self::next_size(self.dimensions.width, required_width);
        let new_height = Self::next_size(self.dimensions.height, required_height);

        let new_atlas_dimensions = AtlasDimensions {
            width: new_width,
            height: new_height,
            ..self.dimensions
        };

        let mut new_atlas = Self {
            data: vec![0u8; new_width * new_height * 4],
            dimensions: new_atlas_dimensions,
            has_color_glyphs: false,
        };

        self.copy_to(&mut new_atlas, 0, 0);

        *self = new_atlas;

        true
    }

    fn next_size(current_size: usize, required_size: usize) -> usize {
        if current_size == 0 {
            required_size
        } else {
            let mut new_size = current_size;

            while required_size > new_size {
                new_size *= 2;
            }

            new_size
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
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

#[derive(Debug, Default, Clone, Copy)]
pub enum GlyphSpan {
    #[default]
    Space,
    Tab,
    Glyph {
        origin_x: f32,
        origin_y: f32,
        x: usize,
        y: usize,
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
    pub fn worse(self, other: Self) -> Self {
        match other {
            Self::Hit => self,
            Self::Miss => {
                if self == Self::Hit {
                    other
                } else {
                    self
                }
            }
            Self::Resize => other,
        }
    }
}

const MAX_ATLAS_WIDTH: usize = 4096;

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
    atlas_used_height: usize,
    atlas_current_row_height: usize,

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
            atlas_used_height: 0,
            atlas_current_row_height: 0,
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

        let sub_atlas = unsafe { text.generate_atlas(glyph) }.unwrap();

        let width = sub_atlas.dimensions.width;
        let height = sub_atlas.dimensions.height;

        if self.atlas_used_width + width > MAX_ATLAS_WIDTH {
            self.atlas_used_height += self.atlas_current_row_height;
            self.atlas_used_width = 0;
            self.atlas_current_row_height = 0;
        }

        let x = self.atlas_used_width;
        let y = self.atlas_used_height;
        let glyph_right = x + width;
        let glyph_bottom = y + height;

        result = result.worse(if self.atlas.ensure_size(glyph_right, glyph_bottom) {
            GlyphCacheResult::Resize
        } else {
            GlyphCacheResult::Miss
        });

        sub_atlas.copy_to(&mut self.atlas, x, y);

        let span = GlyphSpan::Glyph {
            origin_x: sub_atlas.dimensions.origin_x,
            origin_y: sub_atlas.dimensions.origin_y,
            x,
            y,
            width,
            height: sub_atlas.dimensions.height,
            advance: glyph.advance,
            has_color_glyphs: sub_atlas.has_color_glyphs,
        };

        self.glyph_cache.insert(glyph.index, span);

        self.atlas_used_width += width;
        self.atlas_current_row_height = self.atlas_current_row_height.max(height);

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
