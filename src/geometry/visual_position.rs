use super::rect::Rect;

#[derive(Clone, Copy, Debug)]
pub struct VisualPosition {
    pub x: f32,
    pub y: f32,
}

impl VisualPosition {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn offset_by(&self, rect: Rect) -> VisualPosition {
        VisualPosition::new(self.x + rect.x, self.y + rect.y)
    }

    pub fn shift_y(&self, delta: f32) -> VisualPosition {
        VisualPosition::new(self.x, self.y + delta)
    }

    pub fn floor(&self) -> VisualPosition {
        VisualPosition::new(self.x.floor(), self.y.floor())
    }
}
