use std::collections::HashMap;

use crate::input::{action::ActionName, key::Key, keybind::Keybind, mods::Mods};

pub fn new_keymaps() -> HashMap<Keybind, ActionName> {
    [
        (
            Keybind {
                key: Key::P,
                mods: Mods::CTRL,
            },
            ActionName::OpenFileFinder,
        ),
        (
            Keybind {
                key: Key::T,
                mods: Mods::CTRL,
            },
            ActionName::OpenAllFiles,
        ),
        (
            Keybind {
                key: Key::F,
                mods: Mods::CTRL,
            },
            ActionName::OpenSearch,
        ),
        (
            Keybind {
                key: Key::H,
                mods: Mods::CTRL,
            },
            ActionName::OpenSearchAndReplace,
        ),
        (
            Keybind {
                key: Key::F,
                mods: Mods::CTRL | Mods::SHIFT,
            },
            ActionName::OpenFindInFiles,
        ),
        (
            Keybind {
                key: Key::G,
                mods: Mods::CTRL,
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
                mods: Mods::CTRL,
            },
            ActionName::OpenFile,
        ),
        (
            Keybind {
                key: Key::O,
                mods: Mods::CTRL | Mods::SHIFT,
            },
            ActionName::OpenFolder,
        ),
        (
            Keybind {
                key: Key::S,
                mods: Mods::CTRL,
            },
            ActionName::SaveFile,
        ),
        (
            Keybind {
                key: Key::N,
                mods: Mods::CTRL,
            },
            ActionName::NewTab,
        ),
        (
            Keybind {
                key: Key::W,
                mods: Mods::CTRL,
            },
            ActionName::CloseTab,
        ),
        (
            Keybind {
                key: Key::N,
                mods: Mods::CTRL | Mods::ALT,
            },
            ActionName::NewPane,
        ),
        (
            Keybind {
                key: Key::W,
                mods: Mods::CTRL | Mods::ALT,
            },
            ActionName::ClosePane,
        ),
        (
            Keybind {
                key: Key::PageUp,
                mods: Mods::CTRL,
            },
            ActionName::PreviousTab,
        ),
        (
            Keybind {
                key: Key::PageDown,
                mods: Mods::CTRL,
            },
            ActionName::NextTab,
        ),
        (
            Keybind {
                key: Key::PageUp,
                mods: Mods::CTRL | Mods::ALT,
            },
            ActionName::PreviousPane,
        ),
        (
            Keybind {
                key: Key::PageDown,
                mods: Mods::CTRL | Mods::ALT,
            },
            ActionName::NextPane,
        ),
        (
            Keybind {
                key: Key::R,
                mods: Mods::CTRL,
            },
            ActionName::FindReferences,
        ),
        (
            Keybind {
                key: Key::R,
                mods: Mods::CTRL | Mods::SHIFT,
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
                mods: Mods::CTRL,
            },
            ActionName::GoToStart,
        ),
        (
            Keybind {
                key: Key::End,
                mods: Mods::CTRL,
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
                mods: Mods::CTRL,
            },
            ActionName::SelectAll,
        ),
        (
            Keybind {
                key: Key::Z,
                mods: Mods::CTRL,
            },
            ActionName::Undo,
        ),
        (
            Keybind {
                key: Key::Y,
                mods: Mods::CTRL,
            },
            ActionName::Redo,
        ),
        (
            Keybind {
                key: Key::C,
                mods: Mods::CTRL,
            },
            ActionName::Copy,
        ),
        (
            Keybind {
                key: Key::X,
                mods: Mods::CTRL,
            },
            ActionName::Cut,
        ),
        (
            Keybind {
                key: Key::V,
                mods: Mods::CTRL,
            },
            ActionName::Paste,
        ),
        (
            Keybind {
                key: Key::D,
                mods: Mods::CTRL,
            },
            ActionName::AddCursorAtNextOccurance,
        ),
        (
            Keybind {
                key: Key::ForwardSlash,
                mods: Mods::CTRL,
            },
            ActionName::ToggleComments,
        ),
        (
            Keybind {
                key: Key::LBracket,
                mods: Mods::CTRL,
            },
            ActionName::Unindent,
        ),
        (
            Keybind {
                key: Key::RBracket,
                mods: Mods::CTRL,
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
                mods: Mods::CTRL,
            },
            ActionName::MoveLeftWord,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: Mods::CTRL,
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
                mods: Mods::CTRL,
            },
            ActionName::MoveUpParagraph,
        ),
        (
            Keybind {
                key: Key::Down,
                mods: Mods::CTRL,
            },
            ActionName::MoveDownParagraph,
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
                key: Key::Left,
                mods: Mods::ALT,
            },
            ActionName::UndoCursorPosition,
        ),
        (
            Keybind {
                key: Key::Right,
                mods: Mods::ALT,
            },
            ActionName::RedoCursorPosition,
        ),
        (
            Keybind {
                key: Key::Up,
                mods: Mods::CTRL | Mods::ALT,
            },
            ActionName::AddCursorUp,
        ),
        (
            Keybind {
                key: Key::Down,
                mods: Mods::CTRL | Mods::ALT,
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
                mods: Mods::CTRL,
            },
            ActionName::DeleteBackwardWord,
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
                mods: Mods::CTRL,
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
    ]
    .iter()
    .copied()
    .collect()
}
