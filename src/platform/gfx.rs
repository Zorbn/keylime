use std::borrow::Borrow;

use crate::{
    geometry::{
        rect::Rect,
        side::{SIDE_BOTTOM, SIDE_LEFT, SIDE_RIGHT, SIDE_TOP},
    },
    ui::color::Color,
};

use super::{platform_impl, text::AtlasDimensions};

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
        let mut width = 0isize;

        for c in text.into_iter() {
            let c = *c.borrow();

            width += if c == '\t' { TAB_WIDTH as isize } else { 1 };
        }

        width
    }

    pub fn find_x_for_visual_x(
        text: impl IntoIterator<Item = impl Borrow<char>>,
        visual_x: isize,
    ) -> isize {
        let mut current_visual_x = 0isize;
        let mut x = 0isize;

        for c in text.into_iter() {
            let c = *c.borrow();

            current_visual_x += if c == '\t' { TAB_WIDTH as isize } else { 1 };

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
            _ => 1,
        }
    }

    pub fn add_text(
        &mut self,
        text: impl IntoIterator<Item = impl Borrow<char>>,
        x: f32,
        y: f32,
        color: Color,
    ) -> isize {
        let min_char = b' ' as u32;
        let max_char = b'~' as u32;

        let AtlasDimensions {
            width,
            glyph_offset_x,
            glyph_step_x,
            glyph_width,
            glyph_height,
            ..
        } = *self.inner.atlas_dimensions();

        let mut i = 0;

        for c in text.into_iter() {
            let c = *c.borrow();

            let char_index = c as u32;

            if char_index <= min_char || char_index > max_char {
                i += Self::get_char_width(c);
                continue;
            }

            let atlas_char_index = char_index - min_char - 1;

            let mut source_x =
                (glyph_step_x * atlas_char_index as f32 - glyph_offset_x) / width as f32;
            let mut source_width = glyph_step_x / width as f32;

            let mut destination_x = x + i as f32 * glyph_width;
            let mut destination_width = glyph_step_x;

            // DirectWrite might press the first character in the atlas right up against the left edge (eg. the exclamation point),
            // so we'll just shift it back to the center when rendering if necessary.
            if source_x < 0.0 {
                destination_width += source_x * width as f32;
                destination_x -= source_x * width as f32;

                source_width += source_x;
                source_x = 0.0;
            }

            self.inner.add_sprite(
                Rect::new(source_x, 0.0, source_width, 1.0),
                Rect::new(destination_x, y, destination_width, glyph_height),
                color,
            );

            i += Self::get_char_width(c);
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
        self.inner
            .add_sprite(Rect::new(-1.0, -1.0, -1.0, -1.0), rect, color);
    }

    pub fn glyph_width(&self) -> f32 {
        self.inner.atlas_dimensions().glyph_width
    }

    pub fn glyph_height(&self) -> f32 {
        self.inner.atlas_dimensions().glyph_height
    }

    pub fn line_height(&self) -> f32 {
        self.inner.atlas_dimensions().line_height
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
