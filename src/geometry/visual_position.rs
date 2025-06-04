use std::{
    cmp::Ordering,
    ops::{Add, Mul, Sub},
};

use super::rect::Rect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualPosition {
    pub x: f32,
    pub y: f32,
}

impl VisualPosition {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Self {
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

    pub fn lerp_to(&self, other: Self, delta: f32) -> Self {
        Self::new(
            self.x + (other.x - self.x) * delta,
            self.y + (other.y - self.y) * delta,
        )
    }

    pub fn top_left(&self, other: Self) -> Self {
        match self.x.partial_cmp(&other.x) {
            Some(Ordering::Less) => *self,
            Some(Ordering::Greater) => other,
            _ => match self.y.partial_cmp(&other.y) {
                Some(Ordering::Less) => *self,
                _ => other,
            },
        }
    }

    pub fn top_right(&self, other: Self) -> Self {
        match self.x.partial_cmp(&other.x) {
            Some(Ordering::Less) => other,
            Some(Ordering::Greater) => *self,
            _ => match self.y.partial_cmp(&other.y) {
                Some(Ordering::Less) => *self,
                _ => other,
            },
        }
    }

    pub fn bottom_left(&self, other: Self) -> Self {
        match self.x.partial_cmp(&other.x) {
            Some(Ordering::Less) => *self,
            Some(Ordering::Greater) => other,
            _ => match self.y.partial_cmp(&other.y) {
                Some(Ordering::Greater) => *self,
                _ => other,
            },
        }
    }

    pub fn bottom_right(&self, other: Self) -> Self {
        match self.x.partial_cmp(&other.x) {
            Some(Ordering::Less) => other,
            Some(Ordering::Greater) => *self,
            _ => match self.y.partial_cmp(&other.y) {
                Some(Ordering::Greater) => *self,
                _ => other,
            },
        }
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
