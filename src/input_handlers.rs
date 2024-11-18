use crate::{
    keybind::Keybind, mouse_scroll::MouseScroll, mousebind::Mousebind, window_handle::WindowHandle,
};

macro_rules! define_handler {
    ($name:ident, $buffer:ident, $t:ident) => {
        pub struct $name {
            i: isize,
        }

        #[allow(dead_code)]
        impl $name {
            pub fn new(len: usize) -> Self {
                Self {
                    i: len as isize - 1,
                }
            }

            pub fn next(&mut self, window: &mut WindowHandle) -> Option<$t> {
                if self.i < 0 {
                    None
                } else {
                    self.i -= 1;
                    Some(window.$buffer.remove((self.i + 1) as usize))
                }
            }

            pub fn unprocessed(&self, window: &mut WindowHandle, t: $t) {
                window.$buffer.push(t);
            }
        }
    };
}

define_handler!(CharHandler, chars_typed, char);
define_handler!(KeybindHandler, keybinds_typed, Keybind);
define_handler!(MousebindHandler, mousebinds_pressed, Mousebind);
define_handler!(MouseScrollHandler, mouse_scrolls, MouseScroll);
