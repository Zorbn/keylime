use std::ops::{Deref, DerefMut};

use crate::{
    ctx::Ctx,
    text::syntax_highlighter::HighlightKind,
    ui::{
        core::{ContainerDirection, Ui, WidgetLayout},
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

    pub fn update(&mut self, term_list: &mut SlotList<Term>, ctx: &mut Ctx) {
        // ctx.ui
        //     .begin_container(WidgetLayout::default(), ContainerDirection::Horizontal);

        let mut action_handler = ctx.ui.action_handler(ctx.window);

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

        self.inner.update(ctx);

        let focused_tab_index = self.focused_tab_index();

        if let Some((tab, (docs, emulator))) =
            self.get_tab_with_data_mut(focused_tab_index, term_list)
        {
            let doc = emulator.doc_mut(docs);

            let background = ctx
                .config
                .theme
                .highlight_kind_to_color(HighlightKind::Terminal(emulator.background_color));

            tab.update(Some(background), doc, ctx);
        }

        // ctx.ui.end_container();
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
