use crate::{
    config::theme::Theme,
    geometry::visual_position::VisualPosition,
    input::{
        input_handlers::{GraphemeHandler, KeybindHandler, MouseScrollHandler, MousebindHandler},
        keybind::Keybind,
        mods::Mods,
        mouse_scroll::MouseScroll,
        mousebind::Mousebind,
    },
    text::grapheme::GraphemeCursor,
};

use super::{platform_impl, result::Result};

pub struct Window {
    pub(super) inner: platform_impl::window::Window,
}

impl Window {
    #[cfg(test)]
    pub fn new() -> Self {
        Self {
            inner: platform_impl::window::Window::new(),
        }
    }

    pub fn set_theme(&mut self, theme: &Theme) {
        self.inner.set_theme(theme);
    }

    pub fn is_focused(&self) -> bool {
        self.inner.is_focused()
    }

    pub fn was_shown(&self) -> bool {
        #[cfg(target_os = "windows")]
        return true;

        #[cfg(target_os = "macos")]
        self.inner.was_shown
    }

    pub fn grapheme_handler(&self) -> GraphemeHandler {
        self.inner.grapheme_handler()
    }

    pub fn keybind_handler(&self) -> KeybindHandler {
        self.inner.keybind_handler()
    }

    pub fn mousebind_handler(&self) -> MousebindHandler {
        self.inner.mousebind_handler()
    }

    pub fn mouse_scroll_handler(&self) -> MouseScrollHandler {
        self.inner.mouse_scroll_handler()
    }

    pub fn graphemes_typed(&mut self) -> (&str, &mut GraphemeCursor) {
        (&self.inner.graphemes_typed, &mut self.inner.grapheme_cursor)
    }

    pub fn keybinds_typed(&mut self) -> &mut Vec<Keybind> {
        &mut self.inner.keybinds_typed
    }

    pub fn mousebinds_pressed(&mut self) -> &mut Vec<Mousebind> {
        &mut self.inner.mousebinds_pressed
    }

    pub fn mouse_scrolls(&mut self) -> &mut Vec<MouseScroll> {
        &mut self.inner.mouse_scrolls
    }

    pub fn mouse_position(&self) -> VisualPosition {
        self.inner.mouse_position()
    }

    pub fn mods(&self) -> Mods {
        self.inner.mods()
    }

    pub fn set_clipboard(&mut self, text: &str, was_copy_implicit: bool) -> Result<()> {
        self.inner.set_clipboard(text, was_copy_implicit)
    }

    pub fn get_clipboard(&self, text: &mut String) -> Result<()> {
        self.inner.get_clipboard(text)
    }

    pub fn was_copy_implicit(&self) -> bool {
        self.inner.was_copy_implicit()
    }

    pub(super) fn is_char_copy_pastable(c: char) -> bool {
        c != '\r' && c != '\u{200B}'
    }
}
