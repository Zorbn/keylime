use std::ops::{Add, Mul, Sub};

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

    pub fn from_angle(angle: f32) -> Self {
        Self::new(angle.cos(), angle.sin())
    }

    pub fn offset_by(&self, rect: Rect) -> Self {
        Self::new(self.x + rect.x, self.y + rect.y)
    }

    pub fn unoffset_by(&self, rect: Rect) -> Self {
        Self::new(self.x - rect.x, self.y - rect.y)
    }

    pub fn shift_y(&self, delta: f32) -> Self {
        Self::new(self.x, self.y + delta)
    }

    pub fn floor(&self) -> Self {
        Self::new(self.x.floor(), self.y.floor())
    }

    pub fn scale(&self, scale: f32) -> Self {
        Self::new(self.x * scale, self.y * scale)
    }
}

impl Add for VisualPosition {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for VisualPosition {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul for VisualPosition {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.x * rhs.x, self.y * rhs.y)
    }
}
