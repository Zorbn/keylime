use std::{
    env::set_current_dir,
    io,
    ops::{Deref, DerefMut},
    path::Path,
};

use crate::{
    config::Config,
    input::{
        key::Key,
        keybind::{Keybind, MOD_CTRL, MOD_CTRL_SHIFT},
    },
    platform::dialog::{find_file, message, FindFileKind, MessageKind},
    temp_buffer::TempBuffer,
    text::{
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
    ui::{pane::Pane, slot_list::SlotList, widget::Widget, UiHandle},
};

use super::doc_io::{confirm_close, open_or_reuse, reload, try_save};

pub struct EditorPane {
    inner: Pane<Doc>,
}

impl EditorPane {
    pub fn new(doc_list: &mut SlotList<Doc>, line_pool: &mut LinePool) -> Self {
        let mut inner = Pane::new(|doc| doc, |doc| doc);

        let doc_index = doc_list.add(Doc::new(line_pool, None, DocKind::MultiLine));
        inner.add_tab(doc_index, doc_list);

        Self { inner }
    }

    pub fn update(
        &mut self,
        widget: &Widget,
        ui: &mut UiHandle,
        doc_list: &mut SlotList<Doc>,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
    ) {
        let mut keybind_handler = widget.get_keybind_handler(ui);

        while let Some(keybind) = keybind_handler.next(ui.window) {
            match keybind {
                Keybind {
                    key: Key::O,
                    mods: MOD_CTRL,
                } => {
                    if let Ok(path) = find_file(FindFileKind::OpenFile) {
                        if let Err(err) =
                            self.open_file(path.as_path(), doc_list, config, line_pool, time)
                        {
                            message("Error Opening File", &err.to_string(), MessageKind::Ok);
                        }
                    }
                }
                Keybind {
                    key: Key::O,
                    mods: MOD_CTRL_SHIFT,
                } => {
                    if let Ok(path) = find_file(FindFileKind::OpenFolder) {
                        if let Err(err) = set_current_dir(path) {
                            message("Error Opening Folder", &err.to_string(), MessageKind::Ok);
                        }
                    }
                }
                Keybind {
                    key: Key::S,
                    mods: MOD_CTRL,
                } => {
                    if let Some((_, doc)) = self
                        .inner
                        .get_tab_with_data_mut(self.focused_tab_index(), doc_list)
                    {
                        try_save(doc, config, line_pool, time);
                    }
                }
                Keybind {
                    key: Key::N,
                    mods: MOD_CTRL,
                } => {
                    let doc_index = doc_list.add(Doc::new(line_pool, None, DocKind::MultiLine));
                    self.add_tab(doc_index, doc_list, config, line_pool, time);
                }
                Keybind {
                    key: Key::W,
                    mods: MOD_CTRL,
                } => {
                    self.remove_tab(doc_list, config, line_pool, time);
                }
                Keybind {
                    key: Key::R,
                    mods: MOD_CTRL,
                } => {
                    if let Some((tab, doc)) = self
                        .inner
                        .get_tab_with_data_mut(self.focused_tab_index(), doc_list)
                    {
                        reload(doc, config, line_pool, time);
                        tab.camera.recenter();
                    }
                }
                _ => keybind_handler.unprocessed(ui.window, keybind),
            }
        }

        self.inner.update(widget, ui);

        let focused_tab_index = self.focused_tab_index();

        if let Some((tab, doc)) = self.get_tab_with_data_mut(focused_tab_index, doc_list) {
            tab.update(widget, ui, doc, line_pool, text_buffer, config, time);
        }
    }

    pub fn open_file(
        &mut self,
        path: &Path,
        doc_list: &mut SlotList<Doc>,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) -> io::Result<()> {
        let doc_index = open_or_reuse(doc_list, path, line_pool)?;

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
