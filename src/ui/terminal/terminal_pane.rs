use std::ops::{Deref, DerefMut};

use crate::{
    input::{
        key::Key,
        keybind::{Keybind, MOD_CTRL},
    },
    text::line_pool::LinePool,
    ui::{pane::Pane, slot_list::SlotList, widget::Widget, UiHandle},
};

use super::{terminal_emulator::TerminalEmulator, Term, TerminalDocs};

pub struct TerminalPane {
    inner: Pane<Term>,
}

impl TerminalPane {
    pub fn new(term_list: &mut SlotList<Term>, line_pool: &mut LinePool) -> Self {
        let mut inner = Pane::<Term>::new(
            |(docs, emulator)| emulator.get_doc(docs),
            |(docs, emulator)| emulator.get_doc_mut(docs),
        );

        let term = Self::new_term(line_pool);
        let doc_index = term_list.add(term);
        inner.add_tab(doc_index, term_list);

        Self { inner }
    }

    pub fn update(
        &mut self,
        widget: &Widget,
        ui: &mut UiHandle,
        term_list: &mut SlotList<Term>,
        line_pool: &mut LinePool,
    ) {
        let mut keybind_handler = widget.get_keybind_handler(ui);

        while let Some(keybind) = keybind_handler.next(ui.window) {
            match keybind {
                Keybind {
                    key: Key::N,
                    mods: MOD_CTRL,
                } => {
                    let term = Self::new_term(line_pool);
                    let term_index = term_list.add(term);

                    self.add_tab(term_index, term_list);
                }
                Keybind {
                    key: Key::W,
                    mods: MOD_CTRL,
                } => {
                    if let Some(tab) = self.tabs.get(self.focused_tab_index) {
                        let term_index = tab.data_index();

                        self.remove_tab(term_list);

                        if let Some((mut docs, _)) = term_list.remove(term_index) {
                            docs.clear(line_pool);
                        }
                    }
                }
                _ => keybind_handler.unprocessed(ui.window, keybind),
            }
        }

        self.inner.update(widget, ui);
    }

    fn new_term(line_pool: &mut LinePool) -> Term {
        (TerminalDocs::new(line_pool), TerminalEmulator::new())
    }
}

impl Deref for TerminalPane {
    type Target = Pane<Term>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for TerminalPane {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
