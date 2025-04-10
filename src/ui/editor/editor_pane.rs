use std::{
    env::set_current_dir,
    io,
    ops::{Deref, DerefMut},
    path::Path,
};

use crate::{
    config::Config,
    editor_buffers::EditorBuffers,
    platform::dialog::{find_file, message, FindFileKind, MessageKind},
    text::{
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
    ui::{pane::Pane, slot_list::SlotList, widget::WidgetHandle},
};

use super::{
    action_name,
    doc_io::{confirm_close, open_or_reuse, try_save},
};

pub struct EditorPane {
    inner: Pane<Doc>,
}

impl EditorPane {
    pub fn new(doc_list: &mut SlotList<Doc>, line_pool: &mut LinePool) -> Self {
        let mut inner = Pane::new(|doc| doc, |doc| doc);

        let doc_index = doc_list.add(Doc::new(None, line_pool, None, DocKind::MultiLine));
        inner.add_tab(doc_index, doc_list);

        Self { inner }
    }

    pub fn update(
        &mut self,
        widget: &mut WidgetHandle,
        doc_list: &mut SlotList<Doc>,
        buffers: &mut EditorBuffers,
        config: &Config,
        time: f32,
    ) {
        let mut action_handler = widget.get_action_handler();

        while let Some(action) = action_handler.next(widget.window()) {
            match action {
                action_name!(OpenFile) => {
                    if let Ok(path) = find_file(FindFileKind::OpenFile) {
                        if let Err(err) = self.open_file(
                            path.as_path(),
                            doc_list,
                            config,
                            &mut buffers.lines,
                            time,
                        ) {
                            message("Error Opening File", &err.to_string(), MessageKind::Ok);
                        }
                    }
                }
                action_name!(OpenFolder) => {
                    if let Ok(path) = find_file(FindFileKind::OpenFolder) {
                        if let Err(err) = set_current_dir(path) {
                            message("Error Opening Folder", &err.to_string(), MessageKind::Ok);
                        }
                    }
                }
                action_name!(SaveFile) => {
                    if let Some((_, doc)) = self
                        .inner
                        .get_tab_with_data_mut(self.focused_tab_index(), doc_list)
                    {
                        try_save(doc, config, &mut buffers.lines, time);
                    }
                }
                action_name!(NewTab) => {
                    let _ = self.new_file(None, doc_list, config, &mut buffers.lines, time);
                }
                action_name!(CloseTab) => {
                    self.remove_tab(doc_list, config, &mut buffers.lines, time);
                }
                action_name!(ReloadFile) => {
                    if let Some((_, doc)) = self
                        .inner
                        .get_tab_with_data_mut(self.focused_tab_index(), doc_list)
                    {
                        if let Err(err) = doc.reload(buffers, time) {
                            message("Failed to Reload File", &err.to_string(), MessageKind::Ok);
                        }
                    }
                }
                _ => action_handler.unprocessed(widget.window(), action),
            }
        }

        self.inner.update(widget);

        let focused_tab_index = self.focused_tab_index();

        if let Some((tab, doc)) = self.get_tab_with_data_mut(focused_tab_index, doc_list) {
            tab.update(widget, doc, buffers, config, time);
        }
    }

    pub fn new_file(
        &mut self,
        path: Option<&Path>,
        doc_list: &mut SlotList<Doc>,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) -> io::Result<()> {
        let doc = Doc::new(
            path.map(|path| path.to_owned()),
            line_pool,
            None,
            DocKind::MultiLine,
        );

        let doc_index = doc_list.add(doc);

        self.add_tab(doc_index, doc_list, config, line_pool, time);

        Ok(())
    }

    pub fn open_file(
        &mut self,
        path: &Path,
        doc_list: &mut SlotList<Doc>,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) -> io::Result<()> {
        let doc_index = open_or_reuse(doc_list, path, line_pool, time)?;

        self.add_tab(doc_index, doc_list, config, line_pool, time);

        Ok(())
    }

    pub fn add_tab(
        &mut self,
        doc_index: usize,
        doc_list: &mut SlotList<Doc>,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        if let Some(tab_index) = self.get_existing_tab_for_data(doc_index) {
            self.focused_tab_index = tab_index;

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
                self.remove_tab(doc_list, config, line_pool, time);
            }
        }

        self.inner.add_tab(doc_index, doc_list);
    }

    fn remove_tab(
        &mut self,
        doc_list: &mut SlotList<Doc>,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) -> bool {
        let focused_tab_index = self.focused_tab_index();

        let Some((tab, doc)) = self.get_tab_with_data_mut(focused_tab_index, doc_list) else {
            return true;
        };

        let doc_index = tab.data_index();

        if doc.usages() == 1 && !confirm_close(doc, "closing", true, config, line_pool, time) {
            return false;
        }

        self.inner.remove_tab(doc_list);

        if doc_list.get(doc_index).is_some_and(|doc| doc.usages() == 0) {
            if let Some(mut doc) = doc_list.remove(doc_index) {
                doc.clear(line_pool)
            }
        }

        true
    }

    pub fn close_all_tabs(
        &mut self,
        doc_list: &mut SlotList<Doc>,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) -> bool {
        while self.tabs_len() > 0 {
            if !self.remove_tab(doc_list, config, line_pool, time) {
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
