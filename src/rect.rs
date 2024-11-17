use crate::visual_position::VisualPosition;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn contains_position(&self, position: VisualPosition) -> bool {
        position.x >= self.x
            && position.x < self.x + self.width
            && position.y > self.y
            && position.y < self.y + self.height
    }
}
