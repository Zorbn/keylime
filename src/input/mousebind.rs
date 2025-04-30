use super::{mods::Mods, mouse_button::MouseButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseClickKind {
    Single,
    Double,
    Triple,
}

#[derive(Clone, Copy, Debug)]
pub struct Mousebind {
    pub button: Option<MouseButton>,
    pub x: f32,
    pub y: f32,
    pub mods: Mods,
    pub kind: MouseClickKind,
    pub is_drag: bool,
}

impl Mousebind {
    pub fn new(
        button: Option<MouseButton>,
        x: f32,
        y: f32,
        mods: Mods,
        kind: MouseClickKind,
        is_drag: bool,
    ) -> Self {
        Self {
            button,
            x,
            y,
            mods,
            kind,
            is_drag,
        }
    }
}
