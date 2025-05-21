use crate::geometry::position::Position;

#[derive(Clone, Copy)]
pub struct Selection {
    pub start: Position,
    pub end: Position,
}

impl Selection {
    pub fn union(a: Option<Self>, b: Option<Self>) -> Option<Self> {
        let Some(a) = a else {
            return b;
        };

        let Some(b) = b else {
            return Some(a);
        };

        Some(Self {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
        })
    }

    pub fn trim(&self) -> Self {
        let mut result = *self;

        if self.end.y > self.start.y && self.end.x == 0 {
            result.end.y -= 1;
        }

        result
    }
}
