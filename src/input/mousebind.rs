use super::{mods::Mods, mouse_button::MouseButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseClickCount {
    Single,
    Double,
    Triple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseClickKind {
    Press,
    Release,
    Drag,
}

#[derive(Clone, Copy, Debug)]
pub struct Mousebind {
    pub button: Option<MouseButton>,
    pub x: f32,
    pub y: f32,
    pub mods: Mods,
    pub count: MouseClickCount,
    pub kind: MouseClickKind,
}

impl Mousebind {
    pub fn new(
        button: Option<MouseButton>,
        x: f32,
        y: f32,
        mods: Mods,
        count: MouseClickCount,
        kind: MouseClickKind,
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
