use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub x: isize,
    pub y: isize,
}

impl Position {
    pub fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0, y: 0 }
    }

    pub fn min(self, other: Position) -> Position {
        if self < other {
            self
        } else {
            other
        }
    }

    pub fn max(self, other: Position) -> Position {
        if self > other {
            self
        } else {
            other
        }
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.x == other.x && self.y == other.y {
            Ordering::Equal
        } else if self.y < other.y || (self.y == other.y && self.x < other.x) {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}