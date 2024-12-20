use crate::platform::window::Window;

use super::{keybind::Keybind, mouse_scroll::MouseScroll, mousebind::Mousebind};

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

define_handler!(CharHandler, chars_typed, char);
define_handler!(KeybindHandler, keybinds_typed, Keybind);
define_handler!(MousebindHandler, mousebinds_pressed, Mousebind);
define_handler!(MouseScrollHandler, mouse_scrolls, MouseScroll);
