use std::ops::{Deref, DerefMut};

use crate::{
    ctx::Ctx,
    ui::{
        core::{Ui, WidgetId},
        pane::Pane,
        slot_list::SlotList,
    },
};

use super::{action_name, terminal_emulator::TerminalEmulator, Term, TerminalDocs};

pub struct TerminalPane {
    inner: Pane<Term>,
}

impl TerminalPane {
    pub fn new(term_list: &mut SlotList<Term>) -> Self {
        let mut inner = Pane::<Term>::new(
            |(docs, emulator)| emulator.doc(docs),
            |(docs, emulator)| emulator.doc_mut(docs),
        );

        let term = Self::new_term();
        let doc_index = term_list.add(term);
        inner.add_tab(doc_index, term_list);

        Self { inner }
    }

    pub fn update(
        &mut self,
        widget_id: WidgetId,
        ui: &Ui,
        term_list: &mut SlotList<Term>,
        ctx: &mut Ctx,
    ) {
        let mut action_handler = ui.action_handler(widget_id, ctx.window);

        while let Some(action) = action_handler.next(ctx.window) {
            match action {
                action_name!(NewTab) => {
                    let term = Self::new_term();
                    let term_index = term_list.add(term);

                    self.add_tab(term_index, term_list);
                }
                action_name!(CloseTab) => {
                    let focused_tab_index = self.focused_tab_index();

                    if let Some(tab) = self.tabs.get(focused_tab_index) {
                        let term_index = tab.data_index();

                        self.remove_tab(term_list);

                        if let Some((mut docs, _)) = term_list.remove(term_index) {
                            docs.clear(ctx);
                        }
                    }
                }
                _ => action_handler.unprocessed(ctx.window, action),
            }
        }

        self.inner.update(widget_id, ui, ctx.window);
    }

    fn new_term() -> Term {
        (TerminalDocs::new(), TerminalEmulator::new())
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
