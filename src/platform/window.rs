use unicode_segmentation::GraphemeCursor;

use crate::{
    app::App,
    input::{
        action::Action,
        input_handlers::{ActionHandler, GraphemeHandler, MouseScrollHandler, MousebindHandler},
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

    pub fn was_shown(&self) -> bool {
        #[cfg(target_os = "windows")]
        return true;

        #[cfg(target_os = "macos")]
        self.inner.was_shown
    }

    pub fn gfx(&mut self) -> &mut Gfx {
        self.inner.gfx()
    }

    pub fn file_watcher(&mut self) -> &mut FileWatcher {
        self.inner.file_watcher()
    }

    pub fn get_grapheme_handler(&self) -> GraphemeHandler {
        self.inner.get_grapheme_handler()
    }

    pub fn get_action_handler(&self) -> ActionHandler {
        self.inner.get_action_handler()
    }

    pub fn get_mousebind_handler(&self) -> MousebindHandler {
        self.inner.get_mousebind_handler()
    }

    pub fn get_mouse_scroll_handler(&self) -> MouseScrollHandler {
        self.inner.get_mouse_scroll_handler()
    }

    pub fn graphemes_typed(&mut self) -> (&str, &mut GraphemeCursor) {
        (&self.inner.graphemes_typed, &mut self.inner.grapheme_cursor)
    }

    pub fn actions_typed(&mut self) -> &mut Vec<Action> {
        &mut self.inner.actions_typed
    }

    pub fn mousebinds_pressed(&mut self) -> &mut Vec<Mousebind> {
        &mut self.inner.mousebinds_pressed
    }

    pub fn mouse_scrolls(&mut self) -> &mut Vec<MouseScroll> {
        &mut self.inner.mouse_scrolls
    }

    pub fn set_clipboard(&mut self, text: &str, was_copy_implicit: bool) -> Result<()> {
        self.inner.set_clipboard(text, was_copy_implicit)
    }

    pub fn get_clipboard(&mut self) -> Result<&str> {
        self.inner.get_clipboard()
    }

    pub fn was_copy_implicit(&self) -> bool {
        self.inner.was_copy_implicit()
    }

    pub(super) fn is_char_copy_pastable(c: char) -> bool {
        c != '\r' && c != '\0'
    }
}
