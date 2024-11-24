use super::{
    keybind::{MOD_ALT, MOD_CTRL, MOD_SHIFT},
    mouse_button::MouseButton,
};

#[derive(Clone, Copy, Debug)]
pub struct Mousebind {
    pub button: Option<MouseButton>,
    pub x: f32,
    pub y: f32,
    pub mods: u8,
    pub is_drag: bool,
}

impl Mousebind {
    pub fn new(
        button: Option<MouseButton>,
        x: f32,
        y: f32,
        has_shift: bool,
        has_ctrl: bool,
        has_alt: bool,
        is_drag: bool,
    ) -> Self {
        let mut mods = 0u8;

        if has_shift {
            mods |= MOD_SHIFT;
        }

        if has_ctrl {
            mods |= MOD_CTRL;
        }

        if has_alt {
            mods |= MOD_ALT;
        }

        Self {
            button,
            x,
            y,
            mods,
            is_drag,
        }
    }
}
