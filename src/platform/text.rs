use std::{collections::HashMap, ops::RangeInclusive};

use super::{platform_impl, result::Result};

#[derive(Debug, Default, Clone, Copy)]
pub struct AtlasDimensions {
    pub width: usize,
    pub height: usize,
    pub glyph_step_x: usize,
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
pub struct GlyphSpan {
    pub x: usize,
    pub width: usize,
    pub height: usize,
    pub has_color_glyphs: bool,
}

impl GlyphSpan {
    pub fn default_for(atlas: &Atlas, c: char) -> GlyphSpan {
        let index = c as usize - *DEFAULT_GLYPHS.start() as usize;

        GlyphSpan {
            x: index * atlas.dimensions.glyph_step_x,
            width: atlas.dimensions.glyph_step_x,
            height: atlas.dimensions.glyph_height,
            has_color_glyphs: false,
        }
    }
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
    cache: HashMap<char, GlyphSpan>,
    needs_first_resize: bool,
    atlas_used_width: usize,
    pub atlas: Atlas,
}

#[cfg(target_os = "windows")]
const BACKUP_FONT_NAME: &str = "Consolas";

#[cfg(target_os = "macos")]
const BACKUP_FONT_NAME: &str = "Menlo";

const DEFAULT_GLYPHS: RangeInclusive<char> = '!'..='~';

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

        text.atlas = text.generate_atlas(DEFAULT_GLYPHS)?;
        text.atlas_used_width = text.atlas.dimensions.width;

        for c in DEFAULT_GLYPHS {
            text.cache.insert(c, GlyphSpan::default_for(&text.atlas, c));
        }

        Ok(text)
    }

    pub fn get_glyph_span(&mut self, c: char) -> (GlyphSpan, GlyphCacheResult) {
        let mut result = if self.needs_first_resize {
            GlyphCacheResult::Resize
        } else {
            GlyphCacheResult::Hit
        };

        self.needs_first_resize = false;

        if DEFAULT_GLYPHS.contains(&c) {
            return (GlyphSpan::default_for(&self.atlas, c), result);
        }

        if let Some(span) = self.cache.get(&c) {
            return (*span, result);
        }

        let x = self.atlas_used_width;
        let sub_atlas = self.generate_atlas(c..=c).unwrap();
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
            x,
            width: sub_atlas.dimensions.width,
            height: sub_atlas.dimensions.height,
            has_color_glyphs: sub_atlas.has_color_glyphs,
        };

        self.cache.insert(c, span);
        self.atlas_used_width += span.width;

        (span, result)
    }

    fn generate_atlas(&mut self, characters: RangeInclusive<char>) -> Result<Atlas> {
        unsafe { self.inner.generate_atlas(characters) }
    }
}
