use std::vec::Drain;

use crate::{
    config::theme::Theme, geometry::visual_position::VisualPosition, input::mods::Mods,
    ui::msg::Msg,
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

    pub fn msgs(&mut self) -> Drain<'_, Msg> {
        self.inner.msgs()
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
