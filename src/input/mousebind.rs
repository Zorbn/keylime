use super::{mods::Mods, mouse_button::MouseButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseClickCount {
    Single,
    Double,
    Triple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MousebindKind {
    Press,
    Release,
    Move,
}

#[derive(Clone, Copy, Debug)]
pub struct Mousebind {
    pub button: Option<MouseButton>,
    pub x: f32,
    pub y: f32,
    pub mods: Mods,
    pub count: MouseClickCount,
    pub kind: MousebindKind,
}

impl Mousebind {
    pub fn new(
        button: Option<MouseButton>,
        x: f32,
        y: f32,
        mods: Mods,
        count: MouseClickCount,
        kind: MousebindKind,
    ) -> Self {
        Self {
            button,
            x,
            y,
            mods,
            count,
            kind,
        }
    }
}
