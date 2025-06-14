use std::f32::consts::PI;

use crate::{
    geometry::{
        quad::Quad,
        rect::Rect,
        sides::{Side, Sides},
        visual_position::VisualPosition,
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
        self.find_x_for_visual_x_with_clamping(text, visual_x, true)
            .unwrap()
    }

    pub fn find_x_for_visual_x_unclamped(&mut self, text: &str, visual_x: usize) -> Option<usize> {
        self.find_x_for_visual_x_with_clamping(text, visual_x, false)
    }

    fn find_x_for_visual_x_with_clamping(
        &mut self,
        text: &str,
        visual_x: usize,
        do_clamp: bool,
    ) -> Option<usize> {
        let mut current_visual_x = 0;
        let mut x = 0;

        for grapheme in GraphemeIterator::new(text) {
            current_visual_x += self.measure_text(grapheme);

            if current_visual_x > visual_x {
                return Some(x);
            }

            x += grapheme.len();
        }

        (do_clamp || current_visual_x + self.measure_text("\n") > visual_x).then_some(x)
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

        let mut offset = 0;

        for i in glyph_spans.spans_start..glyph_spans.spans_end {
            let span = self.glyph_span(i);

            offset += match span {
                GlyphSpan::Space => 1,
                GlyphSpan::Tab => TAB_WIDTH,
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

                    let destination_x = x + offset as f32 * glyph_width + origin_x;
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
                        )
                        .into(),
                        color,
                        kind,
                    );

                    self.round_glyph_advance(advance)
                }
            };
        }

        offset as f32 * glyph_width
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

            width += match span {
                GlyphSpan::Space => 1,
                GlyphSpan::Tab => TAB_WIDTH,
                GlyphSpan::Glyph { advance, .. } => self.round_glyph_advance(advance),
            };
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

    pub fn add_zig_zag_underline(&mut self, x: f32, y: f32, width: f32, color: Color) {
        let corner_width = self.underline_width();

        let segment_length = self.glyph_width() / 2.0;
        let segment_side_length = segment_length / 2.0 + corner_width;

        let segment_width = corner_width / 2.0;
        let segment_count = (width / segment_length).floor() as usize;

        let x = x + (width - segment_count as f32 * segment_length) / 2.0;

        for i in 0..segment_count {
            let center = VisualPosition::new(x + (i as f32 + 0.5) * segment_length, y);

            let base_angle = if i % 2 == 0 { -PI / 2.0 } else { 0.0 };

            let forward = VisualPosition::from_angle(base_angle + PI / 4.0);
            let right = VisualPosition::from_angle(base_angle + 3.0 * PI / 4.0);

            let center_left = center - forward.scale(segment_side_length);
            let center_right = center + forward.scale(segment_side_length);

            let top_left = center_left - right.scale(segment_width);
            let bottom_left = center_left + right.scale(segment_width);

            let top_right = center_right - right.scale(segment_width);
            let bottom_right = center_right + right.scale(segment_width);

            self.add_sprite(
                Rect::ZERO,
                Quad {
                    top_left,
                    top_right,
                    bottom_left,
                    bottom_right,
                },
                color,
                SpriteKind::Rect,
            );
        }
    }

    pub fn add_underline(&mut self, x: f32, y: f32, width: f32, color: Color) {
        self.add_rect(Rect::new(x, y, width, self.underline_width()), color);
    }

    pub fn add_rect(&mut self, rect: Rect, color: Color) {
        self.add_sprite(Rect::ZERO, rect.into(), color, SpriteKind::Rect);
    }

    pub fn add_quad(&mut self, dst: Quad, color: Color) {
        self.inner
            .add_sprite(Rect::ZERO, dst, color, SpriteKind::Rect);
    }

    fn add_sprite(&mut self, src: Rect, dst: Quad, color: Color, kind: SpriteKind) {
        self.inner.add_sprite(src, dst, color, kind);
    }

    pub fn set_font(&mut self, font_name: &str, font_size: f32) {
        self.inner
            .set_font(font_name, font_size, self.inner.scale());
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

    pub fn line_padding_x(&self) -> f32 {
        self.border_width()
    }

    pub fn line_padding_y(&self) -> f32 {
        ((self.line_height() - self.glyph_height()) / 2.0).ceil()
    }

    pub fn border_width(&self) -> f32 {
        self.inner.scale().floor()
    }

    pub fn underline_width(&self) -> f32 {
        (self.glyph_width() / 8.0).ceil()
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
