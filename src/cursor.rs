use crate::position::Position;

pub struct Cursor {
    pub position: Position,
}

impl Cursor {
    pub fn new(position: Position) -> Self {
        Self { position }
    }
}
