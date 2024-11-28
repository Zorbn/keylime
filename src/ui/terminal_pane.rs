use std::ops::{Deref, DerefMut};

use crate::text::{
    doc::{Doc, DocKind},
    line_pool::LinePool,
};

use super::{doc_list::DocList, pane::Pane, terminal_emulator::TerminalEmulator};

pub struct TerminalPane {
    inner: Pane,
}

impl TerminalPane {
    pub fn new(
        doc_list: &mut DocList,
        emulators: &mut Vec<TerminalEmulator>,
        line_pool: &mut LinePool,
    ) -> Self {
        let mut inner = Pane::new();

        let doc_index = doc_list.add(Doc::new(line_pool, DocKind::Output));
        emulators.push(TerminalEmulator::new());
        inner.add_tab(doc_index, doc_list);

        Self { inner }
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
