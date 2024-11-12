use crate::key::Key;

pub const MOD_SHIFT: u8 = 1 << 0;
pub const MOD_CTRL: u8 = 1 << 1;
pub const MOD_ALT: u8 = 1 << 2;

#[derive(Clone, Copy)]
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

    pub fn key(&self) -> Key {
        self.key
    }

    pub fn has_shift(&self) -> bool {
        (self.mods & MOD_SHIFT) != 0
    }

    pub fn has_ctrl(&self) -> bool {
        (self.mods & MOD_CTRL) != 0
    }

    pub fn has_alt(&self) -> bool {
        (self.mods & MOD_ALT) != 0
    }
}
