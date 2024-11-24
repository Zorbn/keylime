use super::key::Key;

pub const MOD_SHIFT: u8 = 1 << 0;
pub const MOD_CTRL: u8 = 1 << 1;
pub const MOD_ALT: u8 = 1 << 2;

pub const MOD_CTRL_SHIFT: u8 = MOD_CTRL | MOD_SHIFT;
pub const MOD_CTRL_ALT: u8 = MOD_CTRL | MOD_ALT;

#[derive(Clone, Copy, Debug)]
pub struct Keybind {
    pub key: Key,
    pub mods: u8,
}

impl Keybind {
    pub fn new(key: Key, has_shift: bool, has_ctrl: bool, has_alt: bool) -> Self {
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

        Self { mods, key }
    }
}
