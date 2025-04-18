use std::cmp::Ordering;

use crate::geometry::position::Position;

use super::selection::Selection;

#[derive(Clone)]
pub struct Cursor {
    pub position: Position,
    pub selection_anchor: Option<Position>,
    pub desired_visual_x: usize,
}

impl Cursor {
    pub fn new(position: Position, desired_visual_x: usize) -> Self {
        Self {
            position,
            selection_anchor: None,
            desired_visual_x,
        }
    }

    pub fn get_selection(&self) -> Option<Selection> {
        let selection_anchor = self.selection_anchor?;

        match selection_anchor.cmp(&self.position) {
            Ordering::Less => Some(Selection {
                start: selection_anchor,
                end: self.position,
            }),
            Ordering::Greater => Some(Selection {
                start: self.position,
                end: selection_anchor,
            }),
            _ => None,
        }
    }
}
