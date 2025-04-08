use crate::geometry::position::Position;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Done,
    Undone,
    Redone,
}

impl ActionKind {
    pub fn reverse(self) -> Self {
        match self {
            ActionKind::Done | ActionKind::Redone => ActionKind::Undone,
            ActionKind::Undone => ActionKind::Redone,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    SetCursor {
        index: usize,
        position: Position,
        selection_anchor: Option<Position>,
    },
    Insert {
        start: Position,
        end: Position,
    },
    Delete {
        start: Position,
        text_start: usize,
    },
}

const COMBINE_ACTION_TIME: f32 = 0.3;

#[derive(Debug)]
pub struct TimedAction {
    pub action: Action,
    pub time: f32,
}

pub struct ActionHistory {
    actions: Vec<TimedAction>,
    pub deleted_text: String,
}

impl ActionHistory {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            deleted_text: String::new(),
        }
    }

    pub fn clear(&mut self) {
        self.actions.clear();
        self.deleted_text.clear();
    }

    pub fn push_set_cursor(
        &mut self,
        index: usize,
        position: Position,
        selection_anchor: Option<Position>,
        time: f32,
    ) {
        self.actions.push(TimedAction {
            action: Action::SetCursor {
                index,
                position,
                selection_anchor,
            },
            time,
        });
    }

    pub fn push_insert(&mut self, start: Position, end: Position, time: f32) {
        self.actions.push(TimedAction {
            action: Action::Insert { start, end },
            time,
        });
    }

    pub fn push_delete(&mut self, start: Position, text_start: usize, time: f32) {
        self.actions.push(TimedAction {
            action: Action::Delete { start, text_start },
            time,
        });
    }

    pub fn pop(&mut self, last_popped_time: Option<f32>) -> Option<TimedAction> {
        if self.actions.is_empty() {
            return None;
        }

        if let Some(last_popped_time) = last_popped_time {
            if (self.actions.last().unwrap().time - last_popped_time).abs() > COMBINE_ACTION_TIME {
                return None;
            }
        }

        self.actions.pop()
    }
}
