use std::collections::HashMap;

use super::{
    key::Key,
    keybind::Keybind,
    mods::{Mod, Mods},
};

macro_rules! enum_variants {
    ($name:ident, [$($derive:ident),+], $($variant:ident,)*) => {
        #[derive($($derive),+)]
        pub enum $name {
            $($variant,)*
        }

        impl $name {
            pub const VARIANTS: &[$name] = &[
                $($name::$variant,)*
            ];
        }
    };
}

enum_variants!(
    ActionName,
    [Debug, Deserialize, Clone, Copy, PartialEq, Eq],
    Home,
    End,
    GoToStart,
    GoToEnd,
    SelectAll,
    OpenFileExplorer,
    OpenAllActions,
    OpenAllFiles,
    OpenAllDiagnostics,
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
    FindReferences,
    Examine,
);

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
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    pub keybind: Keybind,
    pub name: Option<ActionName>,
}

impl Action {
    pub fn from_keybind(keybind: Keybind) -> Self {
        Self {
            keybind,
            name: None,
        }
    }

    pub fn from_name(name: ActionName) -> Self {
        Self {
            keybind: Keybind::new(Key::Null, Mods::NONE),
            name: Some(name),
        }
    }

    pub fn translate(&self, keymaps: &HashMap<Keybind, ActionName>) -> Self {
        if self.name.is_some() {
            return *self;
        }

        if let Some(action_name) = keymaps.get(&self.keybind) {
            return Self {
                keybind: self.keybind,
                name: Some(*action_name),
            };
        }

        if self.keybind.mods.contains(Mod::Shift) {
            if let Some(action_name) = keymaps.get(&Keybind {
                key: self.keybind.key,
                mods: self.keybind.mods.without(Mod::Shift),
            }) {
                return Self {
                    keybind: self.keybind,
                    name: Some(*action_name),
                };
            }
        }

        *self
    }
}
