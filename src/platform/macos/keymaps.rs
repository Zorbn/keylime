use std::collections::HashMap;

use crate::input::{action::ActionName, key::Key, keybind::Keybind, mods::Mods};

pub fn new_keymaps() -> HashMap<Keybind, ActionName> {
    [
        (
            Keybind {
                key: Key::E,
                mods: Mods::CMD,
            },
            ActionName::OpenFileExplorer,
        ),
        (
            Keybind {
                key: Key::P,
                mods: Mods::CMD,
            },
            ActionName::OpenAllFiles,
        ),
        (
            Keybind {
                key: Key::K,
                mods: Mods::CMD | Mods::SHIFT,
            },
            ActionName::OpenAllDiagnostics,
        ),
        (
            Keybind {
                key: Key::F,
                mods: Mods::CMD,
            },
            ActionName::OpenSearch,
        ),
        (
            Keybind {
                key: Key::H,
                mods: Mods::CMD,
            },
            ActionName::OpenSearchAndReplace,
        ),
        (
            Keybind {
                key: Key::F,
                mods: Mods::CMD | Mods::SHIFT,
            },
            ActionName::OpenFindInFiles,
        ),
        (
            Keybind {
                key: Key::G,
                mods: Mods::CMD,
            },
            ActionName::OpenGoToLine,
        ),
        (
            Keybind {
                key: Key::Grave,
                mods: Mods::CTRL,
            },
            ActionName::FocusTerminal,
        ),
        (
            Keybind {
                key: Key::O,
                mods: Mods::CMD,
            },
            ActionName::OpenFile,
        ),
        (
            Keybind {
                key: Key::O,
                mods: Mods::CMD | Mods::SHIFT,
            },
            ActionName::OpenFolder,
        ),
        (
            Keybind {
                key: Key::S,
                mods: Mods::CMD,
            },
            ActionName::SaveFile,
        ),
        (
            Keybind {
                key: Key::N,
                mods: Mods::CMD,
            },
            ActionName::NewTab,
        ),
        (
            Keybind {
                key: Key::W,
                mods: Mods::CMD,
            },
            ActionName::CloseTab,
        ),
        (
            Keybind {
                key: Key::N,
                mods: Mods::CMD | Mods::ALT,
            },
            ActionName::NewPane,
        ),
        (
            Keybind {
                key: Key::W,
                mods: Mods::CMD | Mods::ALT,
            },
            ActionName::ClosePane,
        ),
        (
            Keybind {
                key: Key::Left,
                mods: Mods::CMD | Mods::ALT,
            },
            ActionName::PreviousTab,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: Mods::CMD | Mods::ALT,
            },
            ActionName::NextTab,
        ),
        (
            Keybind {
                key: Key::LBracket,
                mods: Mods::CMD | Mods::SHIFT,
            },
            ActionName::PreviousTab,
        ),
        (
            Keybind {
                key: Key::RBracket,
                mods: Mods::CMD | Mods::SHIFT,
            },
            ActionName::NextTab,
        ),
        (
            Keybind {
                key: Key::Left,
                mods: Mods::CMD | Mods::SHIFT | Mods::ALT,
            },
            ActionName::PreviousPane,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: Mods::CMD | Mods::SHIFT | Mods::ALT,
            },
            ActionName::NextPane,
        ),
        (
            Keybind {
                key: Key::R,
                mods: Mods::CMD,
            },
            ActionName::FindReferences,
        ),
        (
            Keybind {
                key: Key::R,
                mods: Mods::CMD | Mods::SHIFT,
            },
            ActionName::Rename,
        ),
        (
            Keybind {
                key: Key::Home,
                mods: Mods::NONE,
            },
            ActionName::Home,
        ),
        (
            Keybind {
                key: Key::End,
                mods: Mods::NONE,
            },
            ActionName::End,
        ),
        (
            Keybind {
                key: Key::Home,
                mods: Mods::CMD,
            },
            ActionName::GoToStart,
        ),
        (
            Keybind {
                key: Key::End,
                mods: Mods::CMD,
            },
            ActionName::GoToEnd,
        ),
        (
            Keybind {
                key: Key::Left,
                mods: Mods::CMD,
            },
            ActionName::Home,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: Mods::CMD,
            },
            ActionName::End,
        ),
        (
            Keybind {
                key: Key::Up,
                mods: Mods::CMD,
            },
            ActionName::GoToStart,
        ),
        (
            Keybind {
                key: Key::Down,
                mods: Mods::CMD,
            },
            ActionName::GoToEnd,
        ),
        (
            Keybind {
                key: Key::PageUp,
                mods: Mods::NONE,
            },
            ActionName::PageUp,
        ),
        (
            Keybind {
                key: Key::PageDown,
                mods: Mods::NONE,
            },
            ActionName::PageDown,
        ),
        (
            Keybind {
                key: Key::A,
                mods: Mods::CMD,
            },
            ActionName::SelectAll,
        ),
        (
            Keybind {
                key: Key::Z,
                mods: Mods::CMD,
            },
            ActionName::Undo,
        ),
        (
            Keybind {
                key: Key::Z,
                mods: Mods::CMD | Mods::SHIFT,
            },
            ActionName::Redo,
        ),
        (
            Keybind {
                key: Key::C,
                mods: Mods::CMD,
            },
            ActionName::Copy,
        ),
        (
            Keybind {
                key: Key::X,
                mods: Mods::CMD,
            },
            ActionName::Cut,
        ),
        (
            Keybind {
                key: Key::V,
                mods: Mods::CMD,
            },
            ActionName::Paste,
        ),
        (
            Keybind {
                key: Key::D,
                mods: Mods::CMD,
            },
            ActionName::AddCursorAtNextOccurance,
        ),
        (
            Keybind {
                key: Key::ForwardSlash,
                mods: Mods::CMD,
            },
            ActionName::ToggleComments,
        ),
        (
            Keybind {
                key: Key::LBracket,
                mods: Mods::CMD,
            },
            ActionName::Unindent,
        ),
        (
            Keybind {
                key: Key::RBracket,
                mods: Mods::CMD,
            },
            ActionName::Indent,
        ),
        (
            Keybind {
                key: Key::Left,
                mods: Mods::NONE,
            },
            ActionName::MoveLeft,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: Mods::NONE,
            },
            ActionName::MoveRight,
        ),
        (
            Keybind {
                key: Key::Left,
                mods: Mods::ALT,
            },
            ActionName::MoveLeftWord,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: Mods::ALT,
            },
            ActionName::MoveRightWord,
        ),
        (
            Keybind {
                key: Key::Up,
                mods: Mods::NONE,
            },
            ActionName::MoveUp,
        ),
        (
            Keybind {
                key: Key::Down,
                mods: Mods::NONE,
            },
            ActionName::MoveDown,
        ),
        (
            Keybind {
                key: Key::Up,
                mods: Mods::ALT,
            },
            ActionName::ShiftLinesUp,
        ),
        (
            Keybind {
                key: Key::Down,
                mods: Mods::ALT,
            },
            ActionName::ShiftLinesDown,
        ),
        (
            Keybind {
                key: Key::Minus,
                mods: Mods::CTRL,
            },
            ActionName::UndoCursorPosition,
        ),
        (
            Keybind {
                key: Key::Minus,
                mods: Mods::CTRL | Mods::SHIFT,
            },
            ActionName::RedoCursorPosition,
        ),
        (
            Keybind {
                key: Key::Up,
                mods: Mods::CMD | Mods::ALT,
            },
            ActionName::AddCursorUp,
        ),
        (
            Keybind {
                key: Key::Down,
                mods: Mods::CMD | Mods::ALT,
            },
            ActionName::AddCursorDown,
        ),
        (
            Keybind {
                key: Key::Backspace,
                mods: Mods::NONE,
            },
            ActionName::DeleteBackward,
        ),
        (
            Keybind {
                key: Key::Backspace,
                mods: Mods::ALT,
            },
            ActionName::DeleteBackwardWord,
        ),
        (
            Keybind {
                key: Key::Backspace,
                mods: Mods::CMD,
            },
            ActionName::DeleteBackwardLine,
        ),
        (
            Keybind {
                key: Key::Delete,
                mods: Mods::NONE,
            },
            ActionName::DeleteForward,
        ),
        (
            Keybind {
                key: Key::Delete,
                mods: Mods::ALT,
            },
            ActionName::DeleteForwardWord,
        ),
        (
            Keybind {
                key: Key::Period,
                mods: Mods::CTRL,
            },
            ActionName::RequestCodeAction,
        ),
        (
            Keybind {
                key: Key::K,
                mods: Mods::CMD,
            },
            ActionName::ShowDiagnostic,
        ),
    ]
    .iter()
    .copied()
    .collect()
}
