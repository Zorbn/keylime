use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    // The index of a byte within the line.
    pub x: usize,
    // The index of a line.
    pub y: usize,
}

impl Position {
    pub const ZERO: Self = Self::new(0, 0);

    pub const fn new(x: usize, y: usize) -> Self {
        Self { x, y }
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
