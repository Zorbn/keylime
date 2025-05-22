use std::{
    io,
    ops::{Deref, DerefMut},
    path::Path,
};

use crate::{
    ctx::Ctx,
    normalizable::Normalizable,
    platform::dialog::{find_file, message, FindFileKind, MessageKind},
    text::doc::{Doc, DocFlags},
    ui::{
        core::WidgetId,
        pane::Pane,
        slot_list::{SlotId, SlotList},
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
    pub fn new(doc_list: &mut SlotList<Doc>, parent_id: WidgetId, ctx: &mut Ctx) -> Self {
        let mut inner = Pane::new(|doc| doc, |doc| doc, parent_id, ctx.ui);

        let doc_index = doc_list.add(Doc::new(None, None, DocFlags::MULTI_LINE));
        inner.add_tab(doc_index, doc_list, ctx);

        Self { inner }
    }

    pub fn update(&mut self, doc_list: &mut SlotList<Doc>, ctx: &mut Ctx) {
        let mut keybind_handler = ctx.ui.keybind_handler(self.widget_id(), ctx.window);

        while let Some(action) = keybind_handler.next_action(ctx) {
            match action {
                action_name!(OpenFile) => {
                    if let Ok(path) = find_file(FindFileKind::OpenFile) {
                        if let Err(err) = self.open_file(&path, doc_list, ctx) {
                            message("Error Opening File", &err.to_string(), MessageKind::Ok);
                        }
                    }
                }
                action_name!(SaveFile) => {
                    if let Some((_, doc)) = self.inner.get_focused_tab_with_data_mut(doc_list) {
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
                    if let Some((_, doc)) = self.inner.get_focused_tab_with_data_mut(doc_list) {
                        if let Err(err) = doc.reload(ctx) {
                            message("Failed to Reload File", &err.to_string(), MessageKind::Ok);
                        }
                    }
                }
                _ => keybind_handler.unprocessed(ctx.window, action.keybind),
            }
        }

        self.inner.update(ctx);

        let widget_id = self.widget_id();

        if let Some((tab, doc)) = self.get_focused_tab_with_data_mut(doc_list) {
            tab.update(widget_id, doc, ctx);
        }
    }

    pub fn new_file(
        &mut self,
        path: Option<&Path>,
        doc_list: &mut SlotList<Doc>,
        ctx: &mut Ctx,
    ) -> io::Result<()> {
        let path = path.and_then(|path| path.normalized().ok());

        let mut doc = Doc::new(path, None, DocFlags::MULTI_LINE);
        doc.lsp_did_open("", ctx);

        let doc_id = doc_list.add(doc);

        self.add_tab(doc_id, doc_list, ctx);

        Ok(())
    }

    pub fn open_file(
        &mut self,
        path: &Path,
        doc_list: &mut SlotList<Doc>,
        ctx: &mut Ctx,
    ) -> io::Result<()> {
        let doc_id = open_or_reuse(doc_list, path, ctx)?;

        self.add_tab(doc_id, doc_list, ctx);

        Ok(())
    }

    pub fn add_tab(&mut self, doc_id: SlotId, doc_list: &mut SlotList<Doc>, ctx: &mut Ctx) {
        if let Some(tab_index) = self.get_existing_tab_for_data(doc_id) {
            self.set_focused_tab_index(tab_index);

            return;
        }

        let is_doc_worthless = doc_list.get(doc_id).map(Doc::is_worthless).unwrap_or(false);

        if let Some((_, doc)) = self.get_focused_tab_with_data(doc_list) {
            let is_focused_doc_worthless = doc.is_worthless();

            if !is_doc_worthless && is_focused_doc_worthless {
                self.remove_tab(doc_list, ctx);
            }
        }

        self.inner.add_tab(doc_id, doc_list, ctx);
    }

    fn remove_tab(&mut self, doc_list: &mut SlotList<Doc>, ctx: &mut Ctx) -> bool {
        let Some((tab, doc)) = self.get_focused_tab_with_data_mut(doc_list) else {
            return true;
        };

        let doc_id = tab.data_id();

        if doc.usages() == 1 && !confirm_close(doc, "closing", true, ctx) {
            return false;
        }

        self.inner.remove_tab(doc_list);

        if doc_list.get(doc_id).is_some_and(|doc| doc.usages() == 0) {
            if let Some(mut doc) = doc_list.remove(doc_id) {
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
