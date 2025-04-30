use crate::bit_field::define_bit_field;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Mod {
    Shift,
    Ctrl,
    Alt,
    Cmd,
}

define_bit_field!(Mods, Mod);

impl Mods {
    pub const SHIFT: Self = Self::from(Mod::Shift);
    pub const CTRL: Self = Self::from(Mod::Ctrl);
    pub const ALT: Self = Self::from(Mod::Alt);
    pub const CMD: Self = Self::from(Mod::Cmd);
}
