#[derive(Clone, Copy, Debug)]
pub struct VisualPosition {
    pub x: f32,
    pub y: f32,
}

impl VisualPosition {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}