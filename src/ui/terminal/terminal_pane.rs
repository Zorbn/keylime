use std::ops::{Deref, DerefMut};

use crate::{
    ctx::Ctx,
    ui::{core::WidgetId, pane::Pane, slot_list::SlotList},
};

use super::{action_name, terminal_emulator::TerminalEmulator, Term, TerminalDocs};

pub struct TerminalPane {
    inner: Pane<Term>,
}

impl TerminalPane {
    pub fn new(term_list: &mut SlotList<Term>, parent_id: WidgetId, ctx: &mut Ctx) -> Self {
        let mut inner = Pane::<Term>::new(
            |(docs, emulator)| emulator.doc(docs),
            |(docs, emulator)| emulator.doc_mut(docs),
            parent_id,
            ctx.ui,
        );

        let term = Self::new_term();
        let doc_index = term_list.add(term);
        inner.add_tab(doc_index, term_list, ctx);

        Self { inner }
    }

    pub fn update(&mut self, term_list: &mut SlotList<Term>, ctx: &mut Ctx) {
        let mut action_handler = ctx.ui.action_handler(self.widget_id(), ctx.window);

        while let Some(action) = action_handler.next(ctx) {
            match action {
                action_name!(NewTab) => {
                    let term = Self::new_term();
                    let term_index = term_list.add(term);

                    self.add_tab(term_index, term_list, ctx);
                }
                action_name!(CloseTab) => {
                    if let Some(tab) = self.get_focused_tab() {
                        let term_id = tab.data_id();

                        self.remove_tab(term_list, ctx.ui);

                        if let Some((mut docs, _)) = term_list.remove(term_id) {
                            docs.clear(ctx);
                        }
                    }
                }
                _ => action_handler.unprocessed(ctx.window, action),
            }
        }

        self.inner.update(ctx);
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
