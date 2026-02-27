use std::vec::Drain;

use crate::{
    config::theme::Theme, geometry::visual_position::VisualPosition, input::mods::Mods,
    ui::msg::Msg,
};

use super::result::Result;

pub struct Window {
    pub was_shown: bool,
    msgs: Vec<Msg>,
}

impl Window {
    pub fn new() -> Self {
        Self {
            was_shown: true,
            msgs: Vec::new(),
        }
    }

    pub fn set_theme(&self, _theme: &Theme) {}

    pub fn is_focused(&self) -> bool {
        true
    }

    pub fn msgs(&mut self) -> Drain<'_, Msg> {
        self.msgs.drain(..)
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
