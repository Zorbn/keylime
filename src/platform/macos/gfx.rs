use std::borrow::Borrow;

use crate::{geometry::rect::Rect, ui::color::Color};

use super::{result::Result, window::Window};

pub struct Gfx {}

impl Gfx {
    pub unsafe fn new(font_name: &str, font_size: f32, window: &Window) -> Result<Self> {
        Ok(Gfx {})
    }

    pub unsafe fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        Ok(())
    }

    pub fn update_font(&mut self, font_name: &str, font_size: f32, scale: f32) {}

    pub fn begin_frame(&mut self, clear_color: Color) {}

    pub fn end_frame(&mut self) {}

    pub fn begin(&mut self, bounds: Option<Rect>) {}

    pub fn end(&mut self) {}

    pub fn add_sprite(&mut self, src: Rect, dst: Rect, color: Color) {}

    pub fn measure_text(text: impl IntoIterator<Item = impl Borrow<char>>) -> isize {
        0
    }

    pub fn find_x_for_visual_x(
        text: impl IntoIterator<Item = impl Borrow<char>>,
        visual_x: isize,
    ) -> isize {
        0
    }

    pub fn get_char_width(c: char) -> isize {
        0
    }

    pub fn add_text(
        &mut self,
        text: impl IntoIterator<Item = impl Borrow<char>>,
        x: f32,
        y: f32,
        color: Color,
    ) -> isize {
        0
    }

    pub fn add_bordered_rect(&mut self, rect: Rect, sides: u8, color: Color, border_color: Color) {}

    pub fn add_rect(&mut self, rect: Rect, color: Color) {}

    pub fn glyph_width(&self) -> f32 {
        0.0
    }

    pub fn glyph_height(&self) -> f32 {
        0.0
    }

    pub fn line_height(&self) -> f32 {
        0.0
    }

    pub fn line_padding(&self) -> f32 {
        0.0
    }

    pub fn border_width(&self) -> f32 {
        0.0
    }

    pub fn width(&self) -> f32 {
        0.0
    }

    pub fn height(&self) -> f32 {
        0.0
    }

    pub fn tab_height(&self) -> f32 {
        0.0
    }

    pub fn tab_padding_y(&self) -> f32 {
        0.0
    }

    pub fn height_lines(&self) -> isize {
        0
    }
}
