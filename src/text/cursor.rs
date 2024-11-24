use crate::geometry::position::Position;

use super::selection::Selection;

#[derive(Clone)]
pub struct Cursor {
    pub position: Position,
    pub selection_anchor: Option<Position>,
    pub desired_visual_x: isize,
}

impl Cursor {
    pub fn new(position: Position, desired_visual_x: isize) -> Self {
        Self {
            position,
            selection_anchor: None,
            desired_visual_x,
        }
    }

    pub fn get_selection(&self) -> Option<Selection> {
        let selection_anchor = self.selection_anchor?;

        if selection_anchor < self.position {
            Some(Selection {
                start: selection_anchor,
                end: self.position,
            })
        } else {
            Some(Selection {
                start: self.position,
                end: selection_anchor,
            })
        }
    }
}
