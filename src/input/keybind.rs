use super::{key::Key, mods::Mods};

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
    pub fn new(key: Key, mods: Mods) -> Self {
        let mods = mods.to_bits();

        Self { mods, key }
    }
}
