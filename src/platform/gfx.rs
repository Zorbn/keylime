use std::borrow::Borrow;

use unicode_width::UnicodeWidthChar;

use crate::{
    geometry::{
        rect::Rect,
        side::{SIDE_BOTTOM, SIDE_LEFT, SIDE_RIGHT, SIDE_TOP},
    },
    ui::color::Color,
};

use super::{
    platform_impl,
    text::{AtlasDimensions, GlyphSpan, Glyphs},
};

pub(super) enum SpriteKind {
    Glyph = 0,
    ColorGlyph = 1,
    Rect = 2,
}

const TAB_WIDTH: usize = 4;

pub struct Gfx {
    pub(super) inner: platform_impl::gfx::Gfx,
}

impl Gfx {
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

    pub fn measure_text(text: impl IntoIterator<Item = impl Borrow<char>>) -> isize {
        text.into_iter()
            .map(|c| Self::get_char_width(*c.borrow()))
            .sum()
    }

    pub fn find_x_for_visual_x(
        text: impl IntoIterator<Item = impl Borrow<char>>,
        visual_x: isize,
    ) -> isize {
        let mut current_visual_x = 0isize;
        let mut x = 0isize;

        for c in text.into_iter() {
            let c = *c.borrow();

            current_visual_x += Gfx::get_char_width(c);

            if current_visual_x > visual_x {
                return x;
            }

            x += 1;
        }

        x
    }

    pub fn get_char_width(c: char) -> isize {
        match c {
            '\t' => TAB_WIDTH as isize,
            '\0' => 0,
            _ => UnicodeWidthChar::width(c).unwrap_or(1) as isize,
        }
    }

    pub fn get_glyphs(
        &mut self,
        // TODO: Look more into name conflict when importing std::borrow::Borrow
        text: impl IntoIterator<Item = impl std::borrow::Borrow<char>>,
    ) -> Glyphs {
        self.inner.get_glyphs(text)
    }

    pub fn get_glyph_span(&mut self, glyph_index: u16, glyph_has_color: bool) -> GlyphSpan {
        self.inner.get_glyph_span(glyph_index, glyph_has_color)
    }

    pub fn add_text(
        &mut self,
        text: impl IntoIterator<Item = impl Borrow<char>>,
        x: f32,
        y: f32,
        color: Color,
    ) -> isize {
        let glyphs = self.get_glyphs(text);

        let AtlasDimensions {
            glyph_width,
            glyph_height,
            ..
        } = *self.inner.atlas_dimensions();

        let mut i = 0;

        // for c in text.into_iter() {
        //     let c = *c.borrow();

        //     if c.is_whitespace() || c.is_control() {
        //         i += Self::get_char_width(c);
        //         continue;
        //     }
        for (glyph_index, glyph_has_color) in glyphs.indices.iter().zip(glyphs.has_color) {
            let span = self.get_glyph_span(*glyph_index, glyph_has_color);

            let kind = if span.has_color_glyphs {
                SpriteKind::ColorGlyph
            } else {
                SpriteKind::Glyph
            };

            let source_x = span.x as f32;
            let source_y = 0.0;
            let source_width = span.width as f32;
            let source_height = span.height as f32;

            let destination_x = x + i as f32 * glyph_width as f32 + span.origin_x;
            let destination_y = y + glyph_height as f32 - span.height as f32 + span.origin_y;
            let destination_width = span.width as f32;
            let destination_height = span.height as f32;

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

            // i += Self::get_char_width(c);
            i += 1;
        }

        i
    }

    pub fn add_bordered_rect(&mut self, rect: Rect, sides: u8, color: Color, border_color: Color) {
        let border_width = self.border_width();

        self.add_rect(rect, border_color);

        let left = rect.x
            + if sides & SIDE_LEFT != 0 {
                border_width
            } else {
                0.0
            };

        let right = rect.x + rect.width
            - if sides & SIDE_RIGHT != 0 {
                border_width
            } else {
                0.0
            };

        let top = rect.y
            + if sides & SIDE_TOP != 0 {
                border_width
            } else {
                0.0
            };

        let bottom = rect.y + rect.height
            - if sides & SIDE_BOTTOM != 0 {
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
        ((self.tab_height() - self.line_height()) * 0.75).ceil()
    }

    pub fn height_lines(&self) -> isize {
        (self.height() / self.line_height()) as isize
    }
}
