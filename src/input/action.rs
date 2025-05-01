use std::collections::HashMap;

use super::{keybind::Keybind, mods::Mod};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ActionName {
    Home,
    End,
    GoToStart,
    GoToEnd,
    SelectAll,
    OpenFileFinder,
    OpenAllFiles,
    OpenSearch,
    OpenSearchAndReplace,
    OpenFindInFiles,
    OpenGoToLine,
    OpenFile,
    OpenFolder,
    SaveFile,
    NewTab,
    CloseTab,
    NewPane,
    ClosePane,
    NextTab,
    PreviousTab,
    NextPane,
    PreviousPane,
    ReloadFile,
    FocusTerminal,
    PageUp,
    PageDown,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    AddCursorAtNextOccurance,
    AddCursorUp,
    AddCursorDown,
    ToggleComments,
    Indent,
    Unindent,
    MoveLeft,
    MoveRight,
    MoveLeftWord,
    MoveRightWord,
    MoveUp,
    MoveDown,
    MoveUpParagraph,
    MoveDownParagraph,
    ShiftLinesUp,
    ShiftLinesDown,
    UndoCursorPosition,
    RedoCursorPosition,
    DeleteBackward,
    DeleteBackwardWord,
    DeleteBackwardLine,
    DeleteForward,
    DeleteForwardWord,
    RequestCodeAction,
    Rename,
}

macro_rules! action_name {
    ($name:ident) => {
        $crate::input::action::Action {
            name: Some($crate::input::action::ActionName::$name),
            ..
        }
    };
    ($name:ident, $mods:ident) => {
        $crate::input::action::Action {
            keybind: $crate::input::keybind::Keybind { mods: $mods, .. },
            name: Some($crate::input::action::ActionName::$name),
        }
    };
    (names: $names:pat) => {
        $crate::input::action::Action { name: $names, .. }
    };
}

macro_rules! action_keybind {
    (key: $key:ident, mods: $mods:pat) => {
        $crate::input::action::Action {
            keybind: $crate::input::keybind::Keybind {
                key: $crate::input::key::Key::$key,
                mods: $mods,
            },
            ..
        }
    };
    (key: $key:ident, $mods:ident) => {
        $crate::input::action::Action {
            keybind: $crate::input::keybind::Keybind {
                key: $crate::input::key::Key::$key,
                mods: $mods,
            },
            ..
        }
    };
    (keys: $keys:pat, $mods:ident) => {
        $crate::input::action::Action {
            keybind: $crate::input::keybind::Keybind {
                key: $keys,
                mods: $mods,
            },
            ..
        }
    };
    (key: $key:ident) => {
        $crate::input::action::Action {
            keybind: $crate::input::keybind::Keybind {
                key: $crate::input::key::Key::$key,
                ..
            },
            ..
        }
    };
    ($key:ident, mods: $mods:pat) => {
        $crate::input::action::Action {
            keybind: $crate::input::keybind::Keybind {
                key: $key,
                mods: $mods,
            },
            ..
        }
    };
}

pub(crate) use action_keybind;
pub(crate) use action_name;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    pub keybind: Keybind,
    pub name: Option<ActionName>,
}

impl Action {
    pub fn from_keybind(keybind: Keybind, keymaps: &HashMap<Keybind, ActionName>) -> Self {
        if let Some(action_name) = keymaps.get(&keybind) {
            return Self {
                keybind,
                name: Some(*action_name),
            };
        }

        if keybind.mods.contains(Mod::Shift) {
            if let Some(action_name) = keymaps.get(&Keybind {
                key: keybind.key,
                mods: keybind.mods.without(Mod::Shift),
            }) {
                return Self {
                    keybind,
                    name: Some(*action_name),
                };
            }
        }

        Self {
            keybind,
            name: None,
        }
    }
}
