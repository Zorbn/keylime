use super::{key::Key, mods::Mods};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Keybind {
    pub key: Key,
    pub mods: Mods,
}

impl Keybind {
    pub fn new(key: Key, mods: Mods) -> Self {
        Self { key, mods }
    }
}
