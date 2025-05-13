use crate::{
    geometry::rect::Rect,
    platform::{
        gfx::SpriteKind,
        text_cache::{AtlasDimensions, GlyphSpan, GlyphSpans},
    },
    ui::color::Color,
};

pub struct Gfx;

impl Gfx {
    pub fn begin_frame(&mut self, _clear_color: Color) {}

    pub fn end_frame(&mut self) {}

    pub fn begin(&mut self, _bounds: Option<Rect>) {}

    pub fn end(&mut self) {}

    pub fn glyph_spans(&mut self, _text: &str) -> GlyphSpans {
        GlyphSpans {
            spans_start: 0,
            spans_end: 0,
        }
    }

    pub fn glyph_span(&mut self, _index: usize) -> GlyphSpan {
        GlyphSpan::Space
    }

    pub fn add_sprite(&mut self, _src: Rect, _dst: Rect, _color: Color, _kind: SpriteKind) {}

    pub fn update_font(&mut self, _font_name: &str, _font_size: f32, _scale: f32) {}

    pub fn scale(&self) -> f32 {
        0.0
    }

    pub fn atlas_dimensions(&self) -> AtlasDimensions {
        AtlasDimensions::default()
    }

    pub fn width(&self) -> f32 {
        0.0
    }

    pub fn height(&self) -> f32 {
        0.0
    }
}
