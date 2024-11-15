use crate::position::Position;

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
}
