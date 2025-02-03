use std::{
    cell::RefCell,
    collections::HashMap,
    hash::{Hash, Hasher},
    mem::swap,
    rc::Rc,
};

use crate::text::text_trait;

use super::{platform_impl, result::Result};

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
pub struct Glyph {
    pub index: u16,
    pub has_color: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Glyphs {
    pub indices_start: usize,
    pub indices_end: usize,
    // TODO: Bit vector to save space?
    pub has_color_start: usize,
    pub has_color_end: usize,
}

struct CachedText {
    data: Rc<RefCell<Vec<char>>>,
    start: usize,
    end: usize,
}

impl Hash for CachedText {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let data = self.data.borrow();

        for c in &data[self.start..self.end] {
            c.hash(state);
        }
    }
}

impl PartialEq for CachedText {
    fn eq(&self, other: &Self) -> bool {
        let data = self.data.borrow();
        let other_data = other.data.borrow();

        data[self.start..self.end] == other_data[other.start..other.end]
    }
}

impl Eq for CachedText {}

#[derive(Debug, Clone, Copy)]
pub struct GlyphSpan {
    pub origin_x: f32,
    pub origin_y: f32,
    pub x: usize,
    pub width: usize,
    pub height: usize,
    pub has_color_glyphs: bool,
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

pub struct Text {
    inner: platform_impl::text::Text,

    glyph_cache: HashMap<u16, GlyphSpan>,

    last_glyph_indices: Vec<u16>,
    last_glyph_has_colors: Vec<bool>,

    glyph_indices: Vec<u16>,
    glyph_has_colors: Vec<bool>,

    last_text_cache_data: Rc<RefCell<Vec<char>>>,
    text_cache_data: Rc<RefCell<Vec<char>>>,
    last_text_cache: HashMap<CachedText, Glyphs>,
    text_cache: HashMap<CachedText, Glyphs>,

    needs_first_resize: bool,
    atlas_used_width: usize,
    pub atlas: Atlas,
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

        let mut text = Self {
            inner,

            glyph_cache: HashMap::new(),

            last_glyph_indices: Vec::new(),
            last_glyph_has_colors: Vec::new(),

            glyph_indices: Vec::new(),
            glyph_has_colors: Vec::new(),

            last_text_cache_data: Rc::new(RefCell::new(Vec::new())),
            text_cache_data: Rc::new(RefCell::new(Vec::new())),
            last_text_cache: HashMap::new(),
            text_cache: HashMap::new(),

            needs_first_resize: true,
            atlas_used_width: 0,
            atlas: Atlas::default(),
        };

        let m_glyphs = text.get_glyphs("M".chars());
        let m_glyph = text.get_glyph(&m_glyphs, 0);
        text.atlas = text.generate_atlas(m_glyph)?;

        Ok(text)
    }

    pub fn get_glyphs(&mut self, text: text_trait!()) -> Glyphs {
        let mut text_cache_data = self.text_cache_data.borrow_mut();

        let data_start = text_cache_data.len();

        for c in text.clone() {
            let c = *c.borrow();
            text_cache_data.push(c);
        }

        let data_end = text_cache_data.len();

        let cached_text = CachedText {
            data: self.text_cache_data.clone(),
            start: data_start,
            end: data_end,
        };

        drop(text_cache_data);

        if let Some(glyphs) = self.text_cache.get(&cached_text) {
            let mut text_cache_data = self.text_cache_data.borrow_mut();
            text_cache_data.truncate(data_start);

            return *glyphs;
        }

        let glyphs = if let Some(glyphs) = self.last_text_cache.get(&cached_text) {
            let indices_start = self.glyph_indices.len();
            let has_color_start = self.glyph_has_colors.len();

            self.glyph_indices.extend_from_slice(
                &self.last_glyph_indices[glyphs.indices_start..glyphs.indices_end],
            );
            self.glyph_has_colors.extend_from_slice(
                &self.last_glyph_has_colors[glyphs.has_color_start..glyphs.has_color_end],
            );

            let indices_end = self.glyph_indices.len();
            let has_color_end = self.glyph_has_colors.len();

            Glyphs {
                indices_start,
                indices_end,
                has_color_start,
                has_color_end,
            }
        } else {
            unsafe {
                self.inner
                    .get_glyphs(text, &mut self.glyph_indices, &mut self.glyph_has_colors)
            }
        };

        self.text_cache.insert(cached_text, glyphs);

        glyphs
    }

    pub fn get_glyph_span(&mut self, glyph: Glyph) -> (GlyphSpan, GlyphCacheResult) {
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
        let sub_atlas = self.generate_atlas(glyph).unwrap();
        let glyph_right = x + sub_atlas.dimensions.width;

        if glyph_right == 0
            || glyph_right > self.atlas.dimensions.width
            || sub_atlas.dimensions.height > self.atlas.dimensions.height
        {
            let mut new_width = self.atlas.dimensions.width.max(sub_atlas.dimensions.width);

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

        let span = GlyphSpan {
            origin_x: sub_atlas.dimensions.origin_x,
            origin_y: sub_atlas.dimensions.origin_y,
            x,
            width: sub_atlas.dimensions.width,
            height: sub_atlas.dimensions.height,
            has_color_glyphs: sub_atlas.has_color_glyphs,
        };

        self.glyph_cache.insert(glyph.index, span);
        self.atlas_used_width += span.width;

        (span, result)
    }

    pub fn get_glyph(&mut self, glyphs: &Glyphs, index: usize) -> Glyph {
        Glyph {
            index: self.glyph_indices[glyphs.indices_start + index],
            has_color: self.glyph_has_colors[glyphs.has_color_start + index],
        }
    }

    pub fn swap_caches(&mut self) {
        swap(&mut self.last_glyph_indices, &mut self.glyph_indices);
        swap(&mut self.last_glyph_has_colors, &mut self.glyph_has_colors);

        swap(&mut self.last_text_cache_data, &mut self.text_cache_data);
        swap(&mut self.last_text_cache, &mut self.text_cache);

        self.glyph_indices.clear();
        self.glyph_has_colors.clear();

        self.text_cache_data.borrow_mut().clear();
        self.text_cache.clear();
    }

    fn generate_atlas(&mut self, glyph: Glyph) -> Result<Atlas> {
        unsafe { self.inner.generate_atlas(glyph) }
    }
}
