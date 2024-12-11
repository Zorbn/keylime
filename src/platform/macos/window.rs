use std::path::Path;

use crate::{
    app::App,
    input::{
        input_handlers::{CharHandler, KeybindHandler, MouseScrollHandler, MousebindHandler},
        keybind::Keybind,
        mouse_scroll::MouseScroll,
        mousebind::Mousebind,
    },
};

use super::{file_watcher::FileWatcher, gfx::Gfx, pty::Pty, result::Result};

pub struct WindowRunner {
    app: App,
}

impl WindowRunner {
    pub fn new(app: App) -> Result<Box<Self>> {
        Ok(Box::new(WindowRunner { app }))
    }

    pub fn run(&mut self) {}
}

pub struct Window {
    gfx: Option<Gfx>,
    file_watcher: FileWatcher,

    pub chars_typed: Vec<char>,
    pub keybinds_typed: Vec<Keybind>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,
}

impl Window {
    pub fn update<'a>(
        &mut self,
        is_animating: bool,
        ptys: impl Iterator<Item = &'a Pty>,
        files: impl Iterator<Item = &'a Path>,
    ) -> (f32, f32) {
        (0.0, 0.0)
    }

    pub fn is_running(&self) -> bool {
        true
    }

    pub fn is_focused(&self) -> bool {
        true
    }

    pub fn dpi(&self) -> f32 {
        1.0
    }

    pub fn gfx(&mut self) -> &mut Gfx {
        self.gfx.as_mut().unwrap()
    }

    pub fn file_watcher(&self) -> &FileWatcher {
        &self.file_watcher
    }

    pub fn get_char_handler(&self) -> CharHandler {
        CharHandler::new(0)
    }

    pub fn get_keybind_handler(&self) -> KeybindHandler {
        KeybindHandler::new(0)
    }

    pub fn get_mousebind_handler(&self) -> MousebindHandler {
        MousebindHandler::new(0)
    }

    pub fn get_mouse_scroll_handler(&self) -> MouseScrollHandler {
        MouseScrollHandler::new(0)
    }

    pub fn set_clipboard(&mut self, text: &[char], was_copy_implicit: bool) -> Result<()> {
        Ok(())
    }

    pub fn get_clipboard(&mut self) -> Result<&[char]> {
        Ok(&[])
    }

    pub fn was_copy_implicit(&self) -> bool {
        false
    }
}
