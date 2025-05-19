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

use super::result::Result;

pub struct Window {
    pub was_shown: bool,

    pub graphemes_typed: String,
    pub grapheme_cursor: GraphemeCursor,
    pub keybinds_typed: Vec<Keybind>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,
}

impl Window {
    pub fn new() -> Self {
        Self {
            was_shown: true,
            graphemes_typed: String::new(),
            grapheme_cursor: GraphemeCursor::new(0, 0),
            keybinds_typed: Vec::new(),
            mousebinds_pressed: Vec::new(),
            mouse_scrolls: Vec::new(),
        }
    }

    pub fn set_theme(&self, _theme: &Theme) {}

    pub fn is_focused(&self) -> bool {
        true
    }

    pub fn grapheme_handler(&self) -> GraphemeHandler {
        GraphemeHandler::new(GraphemeCursor::new(0, 0))
    }

    pub fn keybind_handler(&self) -> KeybindHandler {
        KeybindHandler::new(0)
    }

    pub fn mousebind_handler(&self) -> MousebindHandler {
        MousebindHandler::new(0)
    }

    pub fn mouse_scroll_handler(&self) -> MouseScrollHandler {
        MouseScrollHandler::new(0)
    }

    pub fn mouse_position(&self) -> VisualPosition {
        VisualPosition::new(0.0, 0.0)
    }

    pub fn mods(&self) -> Mods {
        Mods::NONE
    }

    pub fn set_clipboard(&self, _text: &str, _was_copy_implicit: bool) -> Result<()> {
        Ok(())
    }

    #[allow(clippy::ptr_arg)]
    pub fn get_clipboard(&self, _text: &mut String) -> Result<()> {
        Ok(())
    }

    pub fn was_copy_implicit(&self) -> bool {
        false
    }
}
