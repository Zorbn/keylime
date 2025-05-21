use super::visual_position::VisualPosition;

#[derive(Clone, Copy, Debug, Default)]
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

    pub fn shrink_left_by(&self, other: Self) -> Self {
        let subtracted_width = (other.right() - self.x).max(0.0);

        Self::new(
            self.x + subtracted_width,
            self.y,
            self.width - subtracted_width,
            self.height,
        )
    }

    pub fn shrink_top_by(&self, other: Self) -> Self {
        let subtracted_height = (other.bottom() - self.y).max(0.0);

        Self::new(
            self.x,
            self.y + subtracted_height,
            self.width,
            self.height - subtracted_height,
        )
    }

    pub fn shrink_bottom_by(&self, other: Self) -> Self {
        let height = (self.bottom().min(other.top()) - self.y).max(0.0);

        Self::new(self.x, self.y, self.width, height)
    }

    pub fn shift_y(&self, delta: f32) -> Self {
        Self::new(self.x, self.y + delta, self.width, self.height)
    }

    pub fn below(&self, other: Self) -> Self {
        Self::new(self.x, other.y + other.height, self.width, self.height)
    }

    pub fn at_bottom_of(&self, other: Self) -> Self {
        Self::new(
            self.x,
            other.bottom() - self.height,
            self.width,
            self.height,
        )
    }

    pub fn add_margin(&self, margin: f32) -> Self {
        Self::new(
            self.x - margin,
            self.y - margin,
            self.width + margin * 2.0,
            self.height + margin * 2.0,
        )
    }

    pub fn center_in(&self, other: Self) -> Self {
        self.center_x_in(other).center_y_in(other)
    }

    pub fn center_x_in(&self, other: Self) -> Self {
        Self::new(
            other.x + (other.width - self.width) / 2.0,
            self.y,
            self.width,
            self.height,
        )
    }

    pub fn center_y_in(&self, other: Self) -> Self {
        Self::new(
            self.x,
            other.y + (other.height - self.height) / 2.0,
            self.width,
            self.height,
        )
    }

    pub fn unoffset_by(&self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.width, self.height)
    }

    pub fn offset_by(&self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.width, self.height)
    }

    pub fn expand_width_in(&self, other: Self) -> Self {
        let padding = (other.height - self.height) / 2.0;

        Self::new(
            other.x + padding,
            self.y,
            other.width - padding * 2.0,
            self.height,
        )
    }

    pub fn expand_to_include(&self, other: Self) -> Self {
        let left = self.left().min(other.left());
        let top = self.top().min(other.top());
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());

        Self::new(left, top, right, bottom)
    }

    pub fn contains_position(&self, position: VisualPosition) -> bool {
        position.x >= self.x
            && position.x <= self.x + self.width
            && position.y >= self.y
            && position.y <= self.y + self.height
    }

    pub fn floor(&self) -> Self {
        Self::new(
            self.x.floor(),
            self.y.floor(),
            self.width.floor(),
            self.height.floor(),
        )
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

    pub fn top(&self) -> f32 {
        self.y
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }
}
