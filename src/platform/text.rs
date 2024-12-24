use std::{collections::HashMap, ops::RangeInclusive};

use super::{platform_impl, result::Result};

pub struct Atlas {
    pub data: Vec<u8>,
    pub dimensions: AtlasDimensions,
    pub has_color_glyphs: bool,
}

#[derive(Debug)]
pub struct AtlasDimensions {
    pub width: usize,
    pub height: usize,
    pub glyph_step_x: f32,
    pub glyph_width: f32,
    pub glyph_height: f32,
    pub line_height: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphSpan {
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
            atlas: Atlas {
                data: Vec::new(),
                dimensions: AtlasDimensions {
                    width: 0,
                    height: 0,
                    glyph_step_x: 0.0,
                    glyph_width: 0.0,
                    glyph_height: 0.0,
                    line_height: 0.0,
                },
                has_color_glyphs: false,
            },
        };

        text.atlas = text.generate_atlas(DEFAULT_GLYPHS)?;
        text.atlas_used_width = text.atlas.dimensions.width;

        let glyph_step_x = text.atlas.dimensions.glyph_step_x as usize;

        for c in DEFAULT_GLYPHS {
            let index = c as usize - *DEFAULT_GLYPHS.start() as usize;

            text.cache.insert(
                c,
                GlyphSpan {
                    x: index * glyph_step_x,
                    width: glyph_step_x,
                    height: text.atlas.dimensions.height,
                    has_color_glyphs: false,
                },
            );
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

            let new_atlas_dimensions = AtlasDimensions {
                width: new_width,
                height: new_height,
                glyph_step_x: self.atlas.dimensions.glyph_step_x,
                glyph_width: self.atlas.dimensions.glyph_width,
                glyph_height: self.atlas.dimensions.glyph_height,
                line_height: self.atlas.dimensions.line_height,
            };
            let mut new_atlas_data =
                vec![0u8; new_atlas_dimensions.height * new_atlas_dimensions.width * 4];

            for y in 0..self.atlas.dimensions.height {
                for x in 0..self.atlas.dimensions.width {
                    let i = (x + y * self.atlas.dimensions.width) * 4;

                    let atlas_i = (x + y * new_atlas_dimensions.width) * 4;

                    new_atlas_data[atlas_i] = self.atlas.data[i];
                    new_atlas_data[atlas_i + 1] = self.atlas.data[i + 1];
                    new_atlas_data[atlas_i + 2] = self.atlas.data[i + 2];
                    new_atlas_data[atlas_i + 3] = self.atlas.data[i + 3];
                }
            }

            self.atlas.data = new_atlas_data;
            self.atlas.dimensions = new_atlas_dimensions;

            result = result.worse(GlyphCacheResult::Resize)
        } else {
            result = result.worse(GlyphCacheResult::Miss)
        };

        let offset_x = x;

        for y in 0..sub_atlas.dimensions.height {
            for x in 0..sub_atlas.dimensions.width {
                let i = (x + y * sub_atlas.dimensions.width) * 4;

                let atlas_x = x + offset_x;
                let atlas_i = (atlas_x + y * self.atlas.dimensions.width) * 4;

                self.atlas.data[atlas_i] = sub_atlas.data[i];
                self.atlas.data[atlas_i + 1] = sub_atlas.data[i + 1];
                self.atlas.data[atlas_i + 2] = sub_atlas.data[i + 2];
                self.atlas.data[atlas_i + 3] = sub_atlas.data[i + 3];
            }
        }

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
