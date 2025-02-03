use std::{borrow::Borrow, collections::HashMap};

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

// TODO: Cache these from the previous frame to be used in the next frame (inspired by Zed).
pub struct Glyphs {
    pub indices: Vec<u16>,
    // TODO: Bit vector to save space?
    pub has_color: Vec<bool>,
}

impl Glyphs {
    pub fn iter(&self) -> impl Iterator<Item = Glyph> + '_ {
        self.indices
            .iter()
            .zip(&self.has_color)
            .map(|(index, has_color)| Glyph {
                index: *index,
                has_color: *has_color,
            })
    }

    pub fn get(&self, index: usize) -> Glyph {
        Glyph {
            index: self.indices[index],
            has_color: self.has_color[index],
        }
    }
}

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
    cache: HashMap<u16, GlyphSpan>,
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
            cache: HashMap::new(),
            needs_first_resize: true,
            atlas_used_width: 0,
            atlas: Atlas::default(),
        };

        let m_glyphs = text.get_glyphs("M".chars());
        text.atlas = text.generate_atlas(m_glyphs.get(0))?;

        Ok(text)
    }

    pub fn get_glyphs(&mut self, text: impl IntoIterator<Item = impl Borrow<char>>) -> Glyphs {
        unsafe { self.inner.get_glyphs(text) }
    }

    pub fn get_glyph_span(&mut self, glyph: Glyph) -> (GlyphSpan, GlyphCacheResult) {
        let mut result = if self.needs_first_resize {
            GlyphCacheResult::Resize
        } else {
            GlyphCacheResult::Hit
        };

        self.needs_first_resize = false;

        if let Some(span) = self.cache.get(&glyph.index) {
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

        self.cache.insert(glyph.index, span);
        self.atlas_used_width += span.width;

        (span, result)
    }

    fn generate_atlas(&mut self, glyph: Glyph) -> Result<Atlas> {
        unsafe { self.inner.generate_atlas(glyph) }
    }
}
