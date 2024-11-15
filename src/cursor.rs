use crate::{position::Position, selection::Selection};

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
        let Some(selection_anchor) = self.selection_anchor else {
            return None;
        };

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
