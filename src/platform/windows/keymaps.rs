use std::collections::HashMap;

use crate::input::{
    action::ActionName,
    key::Key,
    keybind::{Keybind, MOD_ALT, MOD_CTRL, MOD_SHIFT},
};

pub fn new_keymaps() -> HashMap<Keybind, ActionName> {
    [
        (
            Keybind {
                key: Key::P,
                mods: MOD_CTRL,
            },
            ActionName::OpenFileFinder,
        ),
        (
            Keybind {
                key: Key::T,
                mods: MOD_CTRL,
            },
            ActionName::OpenAllFiles,
        ),
        (
            Keybind {
                key: Key::F,
                mods: MOD_CTRL,
            },
            ActionName::OpenSearch,
        ),
        (
            Keybind {
                key: Key::H,
                mods: MOD_CTRL,
            },
            ActionName::OpenSearchAndReplace,
        ),
        (
            Keybind {
                key: Key::F,
                mods: MOD_CTRL | MOD_SHIFT,
            },
            ActionName::OpenFindInFiles,
        ),
        (
            Keybind {
                key: Key::G,
                mods: MOD_CTRL,
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
                mods: MOD_CTRL,
            },
            ActionName::OpenFile,
        ),
        (
            Keybind {
                key: Key::O,
                mods: MOD_CTRL | MOD_SHIFT,
            },
            ActionName::OpenFolder,
        ),
        (
            Keybind {
                key: Key::S,
                mods: MOD_CTRL,
            },
            ActionName::SaveFile,
        ),
        (
            Keybind {
                key: Key::N,
                mods: MOD_CTRL,
            },
            ActionName::NewTab,
        ),
        (
            Keybind {
                key: Key::W,
                mods: MOD_CTRL,
            },
            ActionName::CloseTab,
        ),
        (
            Keybind {
                key: Key::N,
                mods: MOD_CTRL | MOD_ALT,
            },
            ActionName::NewPane,
        ),
        (
            Keybind {
                key: Key::W,
                mods: MOD_CTRL | MOD_ALT,
            },
            ActionName::ClosePane,
        ),
        (
            Keybind {
                key: Key::PageUp,
                mods: MOD_CTRL,
            },
            ActionName::PreviousTab,
        ),
        (
            Keybind {
                key: Key::PageDown,
                mods: MOD_CTRL,
            },
            ActionName::NextTab,
        ),
        (
            Keybind {
                key: Key::PageUp,
                mods: MOD_CTRL | MOD_ALT,
            },
            ActionName::PreviousPane,
        ),
        (
            Keybind {
                key: Key::PageDown,
                mods: MOD_CTRL | MOD_ALT,
            },
            ActionName::NextPane,
        ),
        (
            Keybind {
                key: Key::R,
                mods: MOD_CTRL,
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
                mods: MOD_CTRL,
            },
            ActionName::GoToStart,
        ),
        (
            Keybind {
                key: Key::End,
                mods: MOD_CTRL,
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
                mods: MOD_CTRL,
            },
            ActionName::SelectAll,
        ),
        (
            Keybind {
                key: Key::Z,
                mods: MOD_CTRL,
            },
            ActionName::Undo,
        ),
        (
            Keybind {
                key: Key::Y,
                mods: MOD_CTRL,
            },
            ActionName::Redo,
        ),
        (
            Keybind {
                key: Key::C,
                mods: MOD_CTRL,
            },
            ActionName::Copy,
        ),
        (
            Keybind {
                key: Key::X,
                mods: MOD_CTRL,
            },
            ActionName::Cut,
        ),
        (
            Keybind {
                key: Key::V,
                mods: MOD_CTRL,
            },
            ActionName::Paste,
        ),
        (
            Keybind {
                key: Key::D,
                mods: MOD_CTRL,
            },
            ActionName::AddCursorAtNextOccurance,
        ),
        (
            Keybind {
                key: Key::ForwardSlash,
                mods: MOD_CTRL,
            },
            ActionName::ToggleComments,
        ),
        (
            Keybind {
                key: Key::LBracket,
                mods: MOD_CTRL,
            },
            ActionName::Unindent,
        ),
        (
            Keybind {
                key: Key::RBracket,
                mods: MOD_CTRL,
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
                mods: MOD_CTRL,
            },
            ActionName::MoveLeftWord,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: MOD_CTRL,
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
                mods: MOD_CTRL,
            },
            ActionName::MoveUpParagraph,
        ),
        (
            Keybind {
                key: Key::Down,
                mods: MOD_CTRL,
            },
            ActionName::MoveDownParagraph,
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
                key: Key::Left,
                mods: MOD_ALT,
            },
            ActionName::UndoCursorPosition,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: MOD_ALT,
            },
            ActionName::RedoCursorPosition,
        ),
        (
            Keybind {
                key: Key::Up,
                mods: MOD_CTRL | MOD_ALT,
            },
            ActionName::AddCursorUp,
        ),
        (
            Keybind {
                key: Key::Down,
                mods: MOD_CTRL | MOD_ALT,
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
                mods: MOD_CTRL,
            },
            ActionName::DeleteBackwardWord,
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
                mods: MOD_CTRL,
            },
            ActionName::DeleteForwardWord,
        ),
    ]
    .iter()
    .copied()
    .collect()
}
