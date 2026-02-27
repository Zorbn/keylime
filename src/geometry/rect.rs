use super::visual_position::VisualPosition;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn from_sides(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
    }

    pub fn shrink_top_by(&self, size: f32) -> Self {
        Self::new(self.x, self.y + size, self.width, self.height - size)
    }

    pub fn scale(&self, scale: f32) -> Self {
        self.scale_x(scale).scale_y(scale)
    }

    pub fn scale_x(&self, scale: f32) -> Self {
        let margin = (scale - 1.0) * self.width;

        Self::new(
            self.x - margin * 0.5,
            self.y,
            self.width + margin,
            self.height,
        )
    }

    pub fn scale_y(&self, scale: f32) -> Self {
        let margin = (scale - 1.0) * self.height;

        Self::new(
            self.x,
            self.y - margin * 0.5,
            self.width,
            self.height + margin,
        )
    }

    pub fn shift_x(&self, delta: f32) -> Self {
        Self::new(self.x + delta, self.y, self.width, self.height)
    }

    pub fn shift_y(&self, delta: f32) -> Self {
        Self::new(self.x, self.y + delta, self.width, self.height)
    }

    pub fn add_margin(&self, margin: f32) -> Self {
        Self::new(
            self.x - margin,
            self.y - margin,
            self.width + margin * 2.0,
            self.height + margin * 2.0,
        )
    }

    pub fn center_x_in(&self, other: Self) -> Self {
        Self::new(
            other.x + (other.width - self.width) / 2.0,
            self.y,
            self.width,
            self.height,
        )
    }

    pub fn relative_to(&self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.width, self.height)
    }

    pub fn contains_position(&self, position: VisualPosition) -> bool {
        position.x >= self.x
            && position.x <= self.x + self.width
            && position.y >= self.y
            && position.y <= self.y + self.height
    }

    pub fn top_border(&self, border_width: f32) -> Self {
        Self::new(self.x, self.y, self.width, border_width)
    }

    pub fn left_border(&self, border_width: f32) -> Self {
        Self::new(self.x, self.y, border_width, self.height)
    }

    pub fn right_border(&self, border_width: f32) -> Self {
        Self::new(
            self.x + self.width - border_width,
            self.y,
            border_width,
            self.height,
        )
    }

    pub fn left(&self) -> f32 {
        self.x
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn center_x(&self) -> f32 {
        self.x + self.width / 2.0
    }

    pub fn top(&self) -> f32 {
        self.y
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn position(&self) -> VisualPosition {
        VisualPosition::new(self.x, self.y)
    }
}
