use crate::{
    app::App,
    input::{
        input_handlers::{CharHandler, KeybindHandler, MouseScrollHandler, MousebindHandler},
        keybind::Keybind,
        mouse_scroll::MouseScroll,
        mousebind::Mousebind,
    },
};

use super::{file_watcher::FileWatcher, gfx::Gfx, platform_impl, result::Result};

pub struct WindowRunner {
    inner: Box<platform_impl::window::WindowRunner>,
}

impl WindowRunner {
    pub fn new(app: App) -> Result<Self> {
        let inner = platform_impl::window::WindowRunner::new(app)?;

        Ok(Self { inner })
    }

    pub fn run(&mut self) {
        self.inner.run();
    }
}

pub struct Window {
    pub(super) inner: platform_impl::window::Window,
}

impl Window {
    pub fn is_focused(&self) -> bool {
        self.inner.is_focused()
    }

    pub fn gfx(&mut self) -> &mut Gfx {
        self.inner.gfx()
    }

    pub fn file_watcher(&mut self) -> &mut FileWatcher {
        self.inner.file_watcher()
    }

    pub fn get_char_handler(&self) -> CharHandler {
        self.inner.get_char_handler()
    }

    pub fn get_keybind_handler(&self) -> KeybindHandler {
        self.inner.get_keybind_handler()
    }

    pub fn get_mousebind_handler(&self) -> MousebindHandler {
        self.inner.get_mousebind_handler()
    }

    pub fn get_mouse_scroll_handler(&self) -> MouseScrollHandler {
        self.inner.get_mouse_scroll_handler()
    }

    pub fn chars_typed(&mut self) -> &mut Vec<char> {
        &mut self.inner.chars_typed
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

    pub fn set_clipboard(&mut self, text: &[char], was_copy_implicit: bool) -> Result<()> {
        self.inner.set_clipboard(text, was_copy_implicit)
    }

    pub fn get_clipboard(&mut self) -> Result<&[char]> {
        self.inner.get_clipboard()
    }

    pub fn was_copy_implicit(&self) -> bool {
        self.inner.was_copy_implicit()
    }
}
