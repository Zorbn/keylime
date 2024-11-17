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

    pub fn shrink_left_by(&self, other: Rect) -> Rect {
        let other_right = other.x + other.width;
        let subtracted_width = (other_right - self.x).max(0.0);

        Rect::new(
            self.x + subtracted_width,
            self.y,
            self.width - subtracted_width,
            self.height,
        )
    }

    pub fn shrink_top_by(&self, other: Rect) -> Rect {
        let other_bottom = other.y + other.height;
        let subtracted_height = (other_bottom - self.y).max(0.0);

        Rect::new(
            self.x,
            self.y + subtracted_height,
            self.width,
            self.height - subtracted_height,
        )
    }

    pub fn contains_position(&self, position: VisualPosition) -> bool {
        position.x >= self.x
            && position.x < self.x + self.width
            && position.y > self.y
            && position.y < self.y + self.height
    }
}
