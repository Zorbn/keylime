use crate::{ctx::Ctx, platform::window::Window, text::grapheme::GraphemeCursor};

use super::{action::Action, mouse_scroll::MouseScroll, mousebind::Mousebind};

macro_rules! define_handler {
    ($name:ident, $buffer:ident, $t:ident) => {
        pub struct $name {
            i: usize,
            len: usize,
        }

        #[allow(dead_code)]
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

            pub fn unprocessed(&mut self, window: &mut Window, t: $t) {
                window.$buffer().insert(self.i, t);
                self.i += 1;
                self.len += 1;
            }

            pub fn drain(&mut self, window: &mut Window) {
                while self.next(window).is_some() {}
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

    pub fn next(&mut self, ctx: &mut Ctx) -> Option<Action> {
        self.raw
            .next(ctx.window)
            .map(|action| action.translate(&ctx.config.keymaps))
    }

    pub fn unprocessed(&mut self, window: &mut Window, action: Action) {
        self.raw.unprocessed(window, action);
    }

    pub fn drain(&mut self, window: &mut Window) {
        self.raw.drain(window);
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

    pub fn unprocessed(&self, window: &mut Window) {
        let (graphemes_typed, grapheme_cursor) = window.graphemes_typed();
        grapheme_cursor.previous_boundary(graphemes_typed);
    }

    pub fn drain(&mut self, window: &mut Window) {
        while self.next(window).is_some() {}
    }
}
