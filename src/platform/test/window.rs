use crate::{
    config::theme::Theme,
    geometry::visual_position::VisualPosition,
    input::{
        action::Action,
        input_handlers::{ActionHandler, GraphemeHandler, MouseScrollHandler, MousebindHandler},
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
    pub actions_typed: Vec<Action>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,
}

impl Window {
    pub fn new() -> Self {
        Self {
            was_shown: true,
            graphemes_typed: String::new(),
            grapheme_cursor: GraphemeCursor::new(0, 0),
            actions_typed: Vec::new(),
            mousebinds_pressed: Vec::new(),
            mouse_scrolls: Vec::new(),
        }
    }

    pub fn set_theme(&mut self, _theme: &Theme) {}

    pub fn is_focused(&self) -> bool {
        true
    }

    pub fn get_grapheme_handler(&self) -> GraphemeHandler {
        GraphemeHandler::new(GraphemeCursor::new(0, 0))
    }

    pub fn get_action_handler(&self) -> ActionHandler {
        ActionHandler::new(0)
    }

    pub fn get_mousebind_handler(&self) -> MousebindHandler {
        MousebindHandler::new(0)
    }

    pub fn get_mouse_scroll_handler(&self) -> MouseScrollHandler {
        MouseScrollHandler::new(0)
    }

    pub fn get_mouse_position(&self) -> VisualPosition {
        VisualPosition::new(0.0, 0.0)
    }

    pub fn mods(&self) -> Mods {
        Mods::NONE
    }

    pub fn set_clipboard(&mut self, _text: &str, _was_copy_implicit: bool) -> Result<()> {
        Ok(())
    }

    #[allow(clippy::ptr_arg)]
    pub fn get_clipboard(&mut self, _text: &mut String) -> Result<()> {
        Ok(())
    }

    pub fn was_copy_implicit(&self) -> bool {
        false
    }
}
