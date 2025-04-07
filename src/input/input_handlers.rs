use crate::{platform::window::Window, text::grapheme::GraphemeCursor};

use super::{action::Action, mouse_scroll::MouseScroll, mousebind::Mousebind};

macro_rules! define_handler {
    ($name:ident, $buffer:ident, $t:ident) => {
        pub struct $name {
            i: isize,
            len: isize,
        }

        #[allow(dead_code)]
        impl $name {
            pub fn new(len: usize) -> Self {
                Self {
                    i: 0,
                    len: len as isize,
                }
            }

            pub fn next(&mut self, window: &mut Window) -> Option<$t> {
                if self.i < self.len {
                    let result = Some(window.$buffer().remove(self.i as usize));
                    self.len -= 1;

                    result
                } else {
                    None
                }
            }

            pub fn unprocessed(&mut self, window: &mut Window, t: $t) {
                window.$buffer().insert(0, t);
                self.i += 1;
            }
        }
    };
}

define_handler!(ActionHandler, actions_typed, Action);
define_handler!(MousebindHandler, mousebinds_pressed, Mousebind);
define_handler!(MouseScrollHandler, mouse_scrolls, MouseScroll);

pub struct GraphemeHandler {
    grapheme_cursor: GraphemeCursor,
}

impl GraphemeHandler {
    pub fn new(grapheme_cursor: GraphemeCursor) -> Self {
        Self { grapheme_cursor }
    }

    pub fn next<'a>(&mut self, window: &'a mut Window) -> Option<&'a str> {
        let (graphemes_typed, grapheme_cursor) = window.graphemes_typed();

        let start = self.grapheme_cursor.cur_cursor();
        let end = self.grapheme_cursor.next_boundary(graphemes_typed)?;
        let _ = grapheme_cursor.next_boundary(graphemes_typed);

        Some(&graphemes_typed[start..end])
    }

    pub fn unprocessed(&mut self, window: &mut Window) {
        let (graphemes_typed, grapheme_cursor) = window.graphemes_typed();
        let _ = grapheme_cursor.previous_boundary(graphemes_typed);
    }
}
