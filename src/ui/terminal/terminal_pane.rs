use std::ops::{Deref, DerefMut};

use crate::{
    ctx::Ctx,
    ui::{core::WidgetId, msg::Msg, pane::Pane, pane_list::PaneWrapper, slot_list::SlotList},
};

use super::{action_name, terminal_emulator::TerminalEmulator, Term, TerminalDocs};

pub struct TerminalPane {
    inner: Pane<Term>,

    widget_id: WidgetId,
}

impl TerminalPane {
    pub fn new(term_list: &mut SlotList<Term>, parent_id: WidgetId, ctx: &mut Ctx) -> Self {
        let widget_id = ctx.ui.new_widget(parent_id, Default::default());

        let mut inner = Pane::<Term>::new(
            |(docs, emulator)| emulator.doc(docs),
            |(docs, emulator)| emulator.doc_mut(docs),
            widget_id,
            ctx.ui,
        );

        let term = Self::new_term();
        let doc_index = term_list.add(term);
        inner.add_tab(doc_index, term_list, ctx);

        Self { inner, widget_id }
    }

    fn new_term() -> Term {
        (TerminalDocs::new(), TerminalEmulator::new())
    }
}

impl PaneWrapper<Term> for TerminalPane {
    fn receive_msgs(&mut self, term_list: &mut SlotList<Term>, ctx: &mut Ctx) {
        while let Some(msg) = ctx.ui.msg(self.widget_id) {
            match msg {
                Msg::Action(action_name!(NewTab)) => {
                    let term = Self::new_term();
                    let term_index = term_list.add(term);

                    self.add_tab(term_index, term_list, ctx);
                }
                Msg::Action(action_name!(CloseTab)) => {
                    if let Some(tab) = self.get_focused_tab(ctx.ui) {
                        let term_id = tab.data_id();

                        self.remove_tab(term_list, ctx.ui);

                        if let Some((mut docs, _)) = term_list.remove(term_id) {
                            docs.clear(ctx);
                        }
                    }
                }
                _ => ctx.ui.skip(self.widget_id, msg),
            }
        }

        self.inner.receive_msgs(term_list, ctx);
    }

    fn update(&mut self, _term_list: &mut SlotList<Term>, ctx: &mut Ctx) {
        self.inner.update(ctx);
    }

    fn widget_id(&self) -> WidgetId {
        self.widget_id
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
