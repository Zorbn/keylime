use crate::position::Position;

pub struct Cursor {
    pub position: Position,
    pub desired_visual_x: isize,
}

impl Cursor {
    pub fn new(position: Position, desired_visual_x: isize) -> Self {
        Self { position, desired_visual_x }
    }
}
