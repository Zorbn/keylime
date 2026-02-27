use crate::{config::Config, platform::window::Window, text::grapheme::GraphemeCursor};

use super::{action::Action, mouse_scroll::MouseScroll, mousebind::Mousebind};

macro_rules! define_handler {
    ($name:ident, $buffer:ident, $t:ident) => {
        pub struct $name {
            i: usize,
            len: usize,
        }

        impl $name {
            pub fn new(len: usize) -> Self {
                Self { i: 0, len }
            }

            pub fn next(&mut self, window: &mut Window) -> Option<$t> {
                (self.i < self.len).then(|| {
                    let result = window.$buffer().remove(self.i);
                    self.len -= 1;

                    result
                })
            }
        }
    };
}

define_handler!(RawActionHandler, actions_typed, Action);
define_handler!(MousebindHandler, mousebinds_pressed, Mousebind);
define_handler!(MouseScrollHandler, mouse_scrolls, MouseScroll);

pub struct ActionHandler {
    raw: RawActionHandler,
}

impl ActionHandler {
    pub fn new(len: usize) -> Self {
        Self {
            raw: RawActionHandler::new(len),
        }
    }

    pub fn next(&mut self, config: &Config, window: &mut Window) -> Option<Action> {
        self.raw
            .next(window)
            .map(|action| action.translate(&config.keymaps))
    }
}

pub struct GraphemeHandler {
    grapheme_cursor: GraphemeCursor,
}

impl GraphemeHandler {
    pub fn new(grapheme_cursor: GraphemeCursor) -> Self {
        Self { grapheme_cursor }
    }

    pub fn next<'a>(&mut self, window: &'a mut Window) -> Option<&'a str> {
        let (graphemes_typed, grapheme_cursor) = window.graphemes_typed();

        let start = self.grapheme_cursor.index();
        let end = self.grapheme_cursor.next_boundary(graphemes_typed)?;
        grapheme_cursor.next_boundary(graphemes_typed);

        Some(&graphemes_typed[start..end])
    }
}
