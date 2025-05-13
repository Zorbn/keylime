use std::{
    io,
    ops::{Deref, DerefMut},
    path::Path,
};

use crate::{
    ctx::Ctx,
    normalizable::Normalizable,
    platform::dialog::{find_file, message, FindFileKind, MessageKind},
    text::doc::{Doc, DocKind},
    ui::{
        core::{Ui, Widget},
        pane::Pane,
        slot_list::SlotList,
    },
};

use super::{
    action_name,
    doc_io::{confirm_close, open_or_reuse, try_save},
};

pub struct EditorPane {
    inner: Pane<Doc>,
}

impl EditorPane {
    pub fn new(doc_list: &mut SlotList<Doc>) -> Self {
        let mut inner = Pane::new(|doc| doc, |doc| doc);

        let doc_index = doc_list.add(Doc::new(None, None, DocKind::MultiLine));
        inner.add_tab(doc_index, doc_list);

        Self { inner }
    }

    pub fn update(
        &mut self,
        widget: &Widget,
        ui: &mut Ui,
        doc_list: &mut SlotList<Doc>,
        ctx: &mut Ctx,
    ) {
        let mut action_handler = ui.action_handler(widget, ctx.window);

        while let Some(action) = action_handler.next(ctx.window) {
            match action {
                action_name!(OpenFile) => {
                    if let Ok(path) = find_file(FindFileKind::OpenFile) {
                        if let Err(err) = self.open_file(&path, doc_list, ctx) {
                            message("Error Opening File", &err.to_string(), MessageKind::Ok);
                        }
                    }
                }
                action_name!(SaveFile) => {
                    if let Some((_, doc)) = self
                        .inner
                        .get_tab_with_data_mut(self.focused_tab_index(), doc_list)
                    {
                        try_save(doc, ctx);
                    }
                }
                action_name!(NewTab) => {
                    let _ = self.new_file(None, doc_list, ctx);
                }
                action_name!(CloseTab) => {
                    self.remove_tab(doc_list, ctx);
                }
                action_name!(ReloadFile) => {
                    if let Some((_, doc)) = self
                        .inner
                        .get_tab_with_data_mut(self.focused_tab_index(), doc_list)
                    {
                        if let Err(err) = doc.reload(ctx) {
                            message("Failed to Reload File", &err.to_string(), MessageKind::Ok);
                        }
                    }
                }
                _ => action_handler.unprocessed(ctx.window, action),
            }
        }

        self.inner.update(widget, ui, ctx.window);

        let focused_tab_index = self.focused_tab_index();

        if let Some((tab, doc)) = self.get_tab_with_data_mut(focused_tab_index, doc_list) {
            tab.update(widget, ui, doc, ctx);
        }
    }

    pub fn new_file(
        &mut self,
        path: Option<&Path>,
        doc_list: &mut SlotList<Doc>,
        ctx: &mut Ctx,
    ) -> io::Result<()> {
        let path = path.and_then(|path| path.normalized().ok());

        let mut doc = Doc::new(path, None, DocKind::MultiLine);
        doc.lsp_did_open("", ctx);

        let doc_index = doc_list.add(doc);

        self.add_tab(doc_index, doc_list, ctx);

        Ok(())
    }

    pub fn open_file(
        &mut self,
        path: &Path,
        doc_list: &mut SlotList<Doc>,
        ctx: &mut Ctx,
    ) -> io::Result<()> {
        let doc_index = open_or_reuse(doc_list, path, ctx)?;

        self.add_tab(doc_index, doc_list, ctx);

        Ok(())
    }

    pub fn add_tab(&mut self, doc_index: usize, doc_list: &mut SlotList<Doc>, ctx: &mut Ctx) {
        if let Some(tab_index) = self.get_existing_tab_for_data(doc_index) {
            self.set_focused_tab_index(tab_index);

            return;
        }

        let is_doc_worthless = doc_list
            .get(doc_index)
            .map(|doc| doc.is_worthless())
            .unwrap_or(false);

        let focused_tab_index = self.focused_tab_index();

        if let Some((_, doc)) = self.get_tab_with_data(focused_tab_index, doc_list) {
            let is_focused_doc_worthless = doc.is_worthless();

            if !is_doc_worthless && is_focused_doc_worthless {
                self.remove_tab(doc_list, ctx);
            }
        }

        self.inner.add_tab(doc_index, doc_list);
    }

    fn remove_tab(&mut self, doc_list: &mut SlotList<Doc>, ctx: &mut Ctx) -> bool {
        let focused_tab_index = self.focused_tab_index();

        let Some((tab, doc)) = self.get_tab_with_data_mut(focused_tab_index, doc_list) else {
            return true;
        };

        let doc_index = tab.data_index();

        if doc.usages() == 1 && !confirm_close(doc, "closing", true, ctx) {
            return false;
        }

        self.inner.remove_tab(doc_list);

        if doc_list.get(doc_index).is_some_and(|doc| doc.usages() == 0) {
            if let Some(mut doc) = doc_list.remove(doc_index) {
                doc.clear(ctx)
            }
        }

        true
    }

    pub fn close_all_tabs(&mut self, doc_list: &mut SlotList<Doc>, ctx: &mut Ctx) -> bool {
        while !self.tabs.is_empty() {
            if !self.remove_tab(doc_list, ctx) {
                return false;
            }
        }

        true
    }
}

impl Deref for EditorPane {
    type Target = Pane<Doc>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for EditorPane {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
