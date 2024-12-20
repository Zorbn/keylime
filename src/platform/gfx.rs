use std::borrow::Borrow;

use crate::{geometry::rect::Rect, ui::color::Color};

use super::platform_impl;

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
        platform_impl::gfx::Gfx::measure_text(text)
    }

    pub fn find_x_for_visual_x(
        text: impl IntoIterator<Item = impl Borrow<char>>,
        visual_x: isize,
    ) -> isize {
        platform_impl::gfx::Gfx::find_x_for_visual_x(text, visual_x)
    }

    pub fn get_char_width(c: char) -> isize {
        platform_impl::gfx::Gfx::get_char_width(c)
    }

    pub fn add_text(
        &mut self,
        text: impl IntoIterator<Item = impl Borrow<char>>,
        x: f32,
        y: f32,
        color: Color,
    ) -> isize {
        self.inner.add_text(text, x, y, color)
    }

    pub fn add_bordered_rect(&mut self, rect: Rect, sides: u8, color: Color, border_color: Color) {
        self.inner
            .add_bordered_rect(rect, sides, color, border_color);
    }

    pub fn add_rect(&mut self, rect: Rect, color: Color) {
        self.inner.add_rect(rect, color);
    }

    pub fn glyph_width(&self) -> f32 {
        self.inner.glyph_width()
    }

    pub fn glyph_height(&self) -> f32 {
        self.inner.glyph_height()
    }

    pub fn line_height(&self) -> f32 {
        self.inner.line_height()
    }

    pub fn line_padding(&self) -> f32 {
        self.inner.line_padding()
    }

    pub fn border_width(&self) -> f32 {
        self.inner.border_width()
    }

    pub fn width(&self) -> f32 {
        self.inner.width()
    }

    pub fn height(&self) -> f32 {
        self.inner.height()
    }

    pub fn tab_height(&self) -> f32 {
        self.inner.tab_height()
    }

    pub fn tab_padding_y(&self) -> f32 {
        self.inner.tab_padding_y()
    }

    pub fn height_lines(&self) -> isize {
        self.inner.height_lines()
    }
}
