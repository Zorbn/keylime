use crate::{
    geometry::{
        rect::Rect,
        sides::{Side, Sides},
    },
    text::grapheme::GraphemeIterator,
    ui::color::Color,
};

use super::{
    platform_impl,
    text_cache::{GlyphSpan, GlyphSpans},
};

pub(super) enum SpriteKind {
    Glyph = 0,
    ColorGlyph = 1,
    Rect = 2,
}

pub const TAB_WIDTH: usize = 4;

pub struct Gfx {
    pub(super) inner: platform_impl::gfx::Gfx,
}

impl Gfx {
    #[cfg(test)]
    pub fn new() -> Self {
        Self {
            inner: platform_impl::gfx::Gfx,
        }
    }

    pub fn begin_frame(&mut self, clear_color: Color) {
        self.inner.begin_frame(clear_color);
    }

    pub fn end_frame(&mut self) {
        self.inner.end_frame();
    }

    pub fn begin(&mut self, bounds: Option<Rect>) {
        self.inner.begin(bounds);
    }

    pub fn end(&mut self) {
        self.inner.end();
    }

    pub fn find_x_for_visual_x(&mut self, text: &str, visual_x: usize) -> usize {
        let mut current_visual_x = 0;
        let mut x = 0;

        for grapheme in GraphemeIterator::new(text) {
            current_visual_x += self.measure_text(grapheme);

            if current_visual_x > visual_x {
                return x;
            }

            x += grapheme.len();
        }

        x
    }

    fn glyph_spans(&mut self, text: &str) -> GlyphSpans {
        self.inner.glyph_spans(text)
    }

    fn glyph_span(&mut self, index: usize) -> GlyphSpan {
        self.inner.glyph_span(index)
    }

    pub fn add_text(&mut self, text: &str, x: f32, y: f32, color: Color) -> f32 {
        let glyph_spans = self.glyph_spans(text);

        let glyph_width = self.glyph_width();
        let glyph_height = self.glyph_height();

        let mut offset = 0.0;

        for i in glyph_spans.spans_start..glyph_spans.spans_end {
            let span = self.glyph_span(i);

            match span {
                GlyphSpan::Space => offset += glyph_width,
                GlyphSpan::Tab => offset += TAB_WIDTH as f32 * glyph_width,
                GlyphSpan::Glyph {
                    origin_x,
                    origin_y,
                    x: span_x,
                    width,
                    height,
                    advance,
                    has_color_glyphs,
                } => {
                    let kind = if has_color_glyphs {
                        SpriteKind::ColorGlyph
                    } else {
                        SpriteKind::Glyph
                    };

                    let source_x = span_x as f32;
                    let source_y = 0.0;
                    let source_width = width as f32;
                    let source_height = height as f32;

                    let destination_x = x + offset + origin_x;
                    let destination_y = y + glyph_height - height as f32 + origin_y;
                    let destination_width = width as f32;
                    let destination_height = height as f32;

                    self.add_sprite(
                        Rect::new(source_x, source_y, source_width, source_height),
                        Rect::new(
                            destination_x,
                            destination_y,
                            destination_width,
                            destination_height,
                        ),
                        color,
                        kind,
                    );

                    offset += self.round_glyph_advance(advance) as f32 * glyph_width;
                }
            }
        }

        offset
    }

    pub fn add_background(&mut self, text: &str, x: f32, y: f32, color: Color) {
        let glyph_width = self.glyph_width();
        let width = self.measure_text(text) as f32 * glyph_width;

        self.add_rect(Rect::new(x, y, width, self.line_height()), color);
    }

    pub fn measure_text(&mut self, text: &str) -> usize {
        let glyph_spans = self.glyph_spans(text);

        let mut width = 0;

        for i in glyph_spans.spans_start..glyph_spans.spans_end {
            let span = self.glyph_span(i);

            match span {
                GlyphSpan::Space => width += 1,
                GlyphSpan::Tab => width += TAB_WIDTH,
                GlyphSpan::Glyph { advance, .. } => {
                    width += self.round_glyph_advance(advance);
                }
            }
        }

        width
    }

    fn round_glyph_advance(&self, advance: usize) -> usize {
        (advance as f32 / self.glyph_width()).round() as usize
    }

    pub fn add_bordered_rect(
        &mut self,
        rect: Rect,
        sides: Sides,
        color: Color,
        border_color: Color,
    ) {
        let border_width = self.border_width();

        self.add_rect(rect, border_color);

        let left = rect.x
            + if sides.contains(Side::Left) {
                border_width
            } else {
                0.0
            };

        let right = rect.x + rect.width
            - if sides.contains(Side::Right) {
                border_width
            } else {
                0.0
            };

        let top = rect.y
            + if sides.contains(Side::Top) {
                border_width
            } else {
                0.0
            };

        let bottom = rect.y + rect.height
            - if sides.contains(Side::Bottom) {
                border_width
            } else {
                0.0
            };

        self.add_rect(Rect::new(left, top, right - left, bottom - top), color);
    }

    pub fn add_rect(&mut self, rect: Rect, color: Color) {
        self.add_sprite(
            Rect::new(-1.0, 0.0, -1.0, -1.0),
            rect,
            color,
            SpriteKind::Rect,
        );
    }

    fn add_sprite(&mut self, src: Rect, dst: Rect, color: Color, kind: SpriteKind) {
        self.inner.add_sprite(src, dst, color, kind);
    }

    pub fn update_font(&mut self, font_name: &str, font_size: f32) {
        self.inner
            .update_font(font_name, font_size, self.inner.scale());
    }

    pub fn glyph_width(&self) -> f32 {
        self.inner.atlas_dimensions().glyph_width as f32
    }

    pub fn glyph_height(&self) -> f32 {
        self.inner.atlas_dimensions().glyph_height as f32
    }

    pub fn line_height(&self) -> f32 {
        self.inner.atlas_dimensions().line_height as f32
    }

    pub fn line_padding(&self) -> f32 {
        ((self.line_height() - self.glyph_height()) / 2.0).ceil()
    }

    pub fn border_width(&self) -> f32 {
        self.inner.scale().floor()
    }

    pub fn width(&self) -> f32 {
        self.inner.width()
    }

    pub fn height(&self) -> f32 {
        self.inner.height()
    }

    pub fn tab_height(&self) -> f32 {
        (self.line_height() * 1.25).ceil()
    }

    pub fn tab_padding_y(&self) -> f32 {
        ((self.tab_height() - self.glyph_height()) / 2.0).ceil()
    }
}
