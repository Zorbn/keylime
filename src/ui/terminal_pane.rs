use std::ops::{Deref, DerefMut};

use crate::{
    input::{
        key::Key,
        keybind::{Keybind, MOD_CTRL},
    },
    text::{
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
};

use super::{
    doc_list::DocList, pane::Pane, terminal_emulator::TerminalEmulator, widget::Widget, UiHandle,
};

pub struct TerminalPane {
    inner: Pane,
}

const TERMINAL_DISPLAY_NAME: Option<&str> = Some("Terminal");

impl TerminalPane {
    pub fn new(
        doc_list: &mut DocList,
        emulators: &mut Vec<TerminalEmulator>,
        line_pool: &mut LinePool,
    ) -> Self {
        let mut inner = Pane::new();

        let doc_index = doc_list.add(Doc::new(line_pool, TERMINAL_DISPLAY_NAME, DocKind::Output));
        emulators.push(TerminalEmulator::new());
        inner.add_tab(doc_index, doc_list);

        Self { inner }
    }

    pub fn update(
        &mut self,
        widget: &Widget,
        ui: &mut UiHandle,
        doc_list: &mut DocList,
        emulators: &mut Vec<TerminalEmulator>,
        line_pool: &mut LinePool,
    ) {
        let mut keybind_handler = widget.get_keybind_handler(ui);

        while let Some(keybind) = keybind_handler.next(ui.window) {
            match keybind {
                Keybind {
                    key: Key::N,
                    mods: MOD_CTRL,
                } => {
                    let doc_index =
                        doc_list.add(Doc::new(line_pool, TERMINAL_DISPLAY_NAME, DocKind::Output));
                    emulators.push(TerminalEmulator::new());

                    self.add_tab(doc_index, doc_list);
                }
                Keybind {
                    key: Key::W,
                    mods: MOD_CTRL,
                } => {
                    if let Some(tab) = self.tabs.get(self.focused_tab_index) {
                        let doc_index = tab.doc_index();

                        self.remove_tab(doc_list);

                        emulators.remove(doc_index);
                        doc_list.remove(doc_index, line_pool);
                    }
                }
                _ => keybind_handler.unprocessed(ui.window, keybind),
            }
        }

        self.inner.update(widget, ui);
    }
}

impl Deref for TerminalPane {
    type Target = Pane;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for TerminalPane {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
