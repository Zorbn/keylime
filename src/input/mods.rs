use super::keybind::{MOD_ALT, MOD_CMD, MOD_CTRL, MOD_SHIFT};

pub struct Mods {
    pub has_shift: bool,
    pub has_ctrl: bool,
    pub has_alt: bool,
    pub has_cmd: bool,
}

impl Mods {
    pub fn to_bits(&self) -> u8 {
        let mut mods = 0u8;

        if self.has_shift {
            mods |= MOD_SHIFT;
        }

        if self.has_ctrl {
            mods |= MOD_CTRL;
        }

        if self.has_alt {
            mods |= MOD_ALT;
        }

        if self.has_cmd {
            mods |= MOD_CMD;
        }

        mods
    }
}
