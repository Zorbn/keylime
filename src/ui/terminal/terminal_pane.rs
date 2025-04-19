use std::ops::{Deref, DerefMut};

use crate::{
    ctx::Ctx,
    text::line_pool::LinePool,
    ui::{
        core::{Ui, Widget},
        pane::Pane,
        slot_list::SlotList,
    },
};

use super::{action_name, terminal_emulator::TerminalEmulator, Term, TerminalDocs};

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
        widget: &mut Widget,
        ui: &mut Ui,
        term_list: &mut SlotList<Term>,
        ctx: &mut Ctx,
    ) {
        let mut action_handler = ui.get_action_handler(widget, ctx.window);

        while let Some(action) = action_handler.next(ctx.window) {
            match action {
                action_name!(NewTab) => {
                    let term = Self::new_term(&mut ctx.buffers.lines);
                    let term_index = term_list.add(term);

                    self.add_tab(term_index, term_list);
                }
                action_name!(CloseTab) => {
                    let focused_tab_index = self.focused_tab_index();

                    if let Some(tab) = self.tabs.get(focused_tab_index) {
                        let term_index = tab.data_index();

                        self.remove_tab(term_list);

                        if let Some((mut docs, _)) = term_list.remove(term_index) {
                            docs.clear(&mut ctx.buffers.lines);
                        }
                    }
                }
                _ => action_handler.unprocessed(ctx.window, action),
            }
        }

        self.inner.update(widget, ui, ctx.window);
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
