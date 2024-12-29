use super::key::Key;

pub const MOD_SHIFT: u8 = 1 << 0;
pub const MOD_CTRL: u8 = 1 << 1;
pub const MOD_ALT: u8 = 1 << 2;
pub const MOD_CMD: u8 = 1 << 3;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Keybind {
    pub key: Key,
    pub mods: u8,
}

impl Keybind {
    pub fn new(key: Key, has_shift: bool, has_ctrl: bool, has_alt: bool, has_cmd: bool) -> Self {
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

        if has_cmd {
            mods |= MOD_CMD;
        }

        Self { mods, key }
    }
}
