use crate::geometry::position::Position;

use super::doc::Doc;

#[derive(Clone, Copy)]
pub struct Selection {
    pub start: Position,
    pub end: Position,
}

impl Selection {
    pub fn union(a: Option<Selection>, b: Option<Selection>) -> Option<Selection> {
        let Some(a) = a else {
            return b;
        };

        let Some(b) = b else {
            return Some(a);
        };

        Some(Selection {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
        })
    }

    pub fn trim_lines_without_selected_chars(&self) -> Selection {
        let mut result = *self;

        if self.end.y > self.start.y && self.end.x == 0 {
            result.end.y -= 1;
        }

        result
    }
}
