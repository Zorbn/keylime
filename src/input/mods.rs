use serde::Deserialize;

use crate::bit_field::define_bit_field;

#[derive(Debug, Deserialize, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Mod {
    Shift,
    Ctrl,
    Alt,
    Cmd,
}

define_bit_field!(Mods, Mod, u8);

impl Mods {
    pub const SHIFT: Self = Self::from(Mod::Shift);
    pub const CTRL: Self = Self::from(Mod::Ctrl);
}
