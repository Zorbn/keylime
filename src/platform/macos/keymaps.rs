use std::collections::HashMap;

use crate::input::{
    action::ActionName,
    key::Key,
    keybind::{Keybind, MOD_ALT, MOD_CMD, MOD_CTRL, MOD_SHIFT},
};

pub fn new_keymaps() -> HashMap<Keybind, ActionName> {
    [
        (
            Keybind {
                key: Key::P,
                mods: MOD_CMD,
            },
            ActionName::OpenCommandPalette,
        ),
        (
            Keybind {
                key: Key::F,
                mods: MOD_CMD,
            },
            ActionName::OpenSearch,
        ),
        (
            Keybind {
                key: Key::H,
                mods: MOD_CMD,
            },
            ActionName::OpenSearchAndReplace,
        ),
        (
            Keybind {
                key: Key::G,
                mods: MOD_CMD,
            },
            ActionName::OpenGoToLine,
        ),
        (
            Keybind {
                key: Key::Grave,
                mods: MOD_CTRL,
            },
            ActionName::FocusTerminal,
        ),
        (
            Keybind {
                key: Key::O,
                mods: MOD_CMD,
            },
            ActionName::OpenFile,
        ),
        (
            Keybind {
                key: Key::O,
                mods: MOD_CMD | MOD_SHIFT,
            },
            ActionName::OpenFolder,
        ),
        (
            Keybind {
                key: Key::S,
                mods: MOD_CMD,
            },
            ActionName::SaveFile,
        ),
        (
            Keybind {
                key: Key::N,
                mods: MOD_CMD,
            },
            ActionName::NewTab,
        ),
        (
            Keybind {
                key: Key::W,
                mods: MOD_CMD,
            },
            ActionName::CloseTab,
        ),
        (
            Keybind {
                key: Key::N,
                mods: MOD_CMD | MOD_ALT,
            },
            ActionName::NewPane,
        ),
        (
            Keybind {
                key: Key::W,
                mods: MOD_CMD | MOD_ALT,
            },
            ActionName::ClosePane,
        ),
        (
            Keybind {
                key: Key::Left,
                mods: MOD_CMD | MOD_ALT,
            },
            ActionName::PreviousTab,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: MOD_CMD | MOD_ALT,
            },
            ActionName::NextTab,
        ),
        (
            Keybind {
                key: Key::R,
                mods: MOD_CMD,
            },
            ActionName::ReloadFile,
        ),
        (
            Keybind {
                key: Key::Home,
                mods: 0,
            },
            ActionName::Home,
        ),
        (
            Keybind {
                key: Key::End,
                mods: 0,
            },
            ActionName::End,
        ),
        (
            Keybind {
                key: Key::Home,
                mods: MOD_CMD,
            },
            ActionName::GoToStart,
        ),
        (
            Keybind {
                key: Key::End,
                mods: MOD_CMD,
            },
            ActionName::GoToEnd,
        ),
        (
            Keybind {
                key: Key::Left,
                mods: MOD_CMD,
            },
            ActionName::Home,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: MOD_CMD,
            },
            ActionName::End,
        ),
        (
            Keybind {
                key: Key::Up,
                mods: MOD_CMD,
            },
            ActionName::GoToStart,
        ),
        (
            Keybind {
                key: Key::Down,
                mods: MOD_CMD,
            },
            ActionName::GoToEnd,
        ),
        (
            Keybind {
                key: Key::PageUp,
                mods: 0,
            },
            ActionName::PageUp,
        ),
        (
            Keybind {
                key: Key::PageDown,
                mods: 0,
            },
            ActionName::PageDown,
        ),
        (
            Keybind {
                key: Key::A,
                mods: MOD_CMD,
            },
            ActionName::SelectAll,
        ),
        (
            Keybind {
                key: Key::Z,
                mods: MOD_CMD,
            },
            ActionName::Undo,
        ),
        (
            Keybind {
                key: Key::Y,
                mods: MOD_CMD,
            },
            ActionName::Redo,
        ),
        (
            Keybind {
                key: Key::C,
                mods: MOD_CMD,
            },
            ActionName::Copy,
        ),
        (
            Keybind {
                key: Key::X,
                mods: MOD_CMD,
            },
            ActionName::Cut,
        ),
        (
            Keybind {
                key: Key::V,
                mods: MOD_CMD,
            },
            ActionName::Paste,
        ),
        (
            Keybind {
                key: Key::D,
                mods: MOD_CMD,
            },
            ActionName::AddCursorAtNextOccurance,
        ),
        (
            Keybind {
                key: Key::ForwardSlash,
                mods: MOD_CMD,
            },
            ActionName::ToggleComments,
        ),
        (
            Keybind {
                key: Key::LBracket,
                mods: MOD_CMD,
            },
            ActionName::Unindent,
        ),
        (
            Keybind {
                key: Key::RBracket,
                mods: MOD_CMD,
            },
            ActionName::Indent,
        ),
        (
            Keybind {
                key: Key::Left,
                mods: 0,
            },
            ActionName::MoveLeft,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: 0,
            },
            ActionName::MoveRight,
        ),
        (
            Keybind {
                key: Key::Left,
                mods: MOD_ALT,
            },
            ActionName::MoveLeftWord,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: MOD_ALT,
            },
            ActionName::MoveRightWord,
        ),
        (
            Keybind {
                key: Key::Up,
                mods: 0,
            },
            ActionName::MoveUp,
        ),
        (
            Keybind {
                key: Key::Down,
                mods: 0,
            },
            ActionName::MoveDown,
        ),
        (
            Keybind {
                key: Key::Up,
                mods: MOD_ALT,
            },
            ActionName::ShiftLinesUp,
        ),
        (
            Keybind {
                key: Key::Down,
                mods: MOD_ALT,
            },
            ActionName::ShiftLinesDown,
        ),
        (
            Keybind {
                key: Key::Up,
                mods: MOD_CMD | MOD_ALT,
            },
            ActionName::AddCursorUp,
        ),
        (
            Keybind {
                key: Key::Down,
                mods: MOD_CMD | MOD_ALT,
            },
            ActionName::AddCursorDown,
        ),
        (
            Keybind {
                key: Key::Backspace,
                mods: 0,
            },
            ActionName::DeleteBackward,
        ),
        (
            Keybind {
                key: Key::Backspace,
                mods: MOD_ALT,
            },
            ActionName::DeleteBackwardWord,
        ),
        (
            Keybind {
                key: Key::Backspace,
                mods: MOD_CMD,
            },
            ActionName::DeleteBackwardLine,
        ),
        (
            Keybind {
                key: Key::Delete,
                mods: 0,
            },
            ActionName::DeleteForward,
        ),
        (
            Keybind {
                key: Key::Delete,
                mods: MOD_ALT,
            },
            ActionName::DeleteForwardWord,
        ),
    ]
    .iter()
    .copied()
    .collect()
}
