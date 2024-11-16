use std::path::Path;

use crate::{
    dialog::{find_file, message, FindFileKind, MessageKind, MessageResponse},
    doc::Doc,
    gfx::Gfx,
    key::Key,
    keybind::{Keybind, MOD_CTRL},
    line_pool::LinePool,
    syntax_highlighter::Syntax,
    tab::Tab,
    temp_buffer::TempBuffer,
    theme::Theme,
    window::Window,
};

pub struct Editor {
    docs: Vec<Option<Doc>>,
    tabs: Vec<Tab>,
    focused_tab_index: usize,
}

impl Editor {
    pub fn new(line_pool: &mut LinePool) -> Self {
        let mut editor = Self {
            docs: Vec::new(),
            tabs: Vec::new(),
            focused_tab_index: 0,
        };

        editor.add_doc(Doc::new(line_pool));
        editor.add_tab(0);

        editor
    }

    pub fn is_animating(&self) -> bool {
        if let Some(tab) = self.tabs.get(self.focused_tab_index) {
            tab.is_animating()
        } else {
            false
        }
    }

    pub fn update(
        &mut self,
        window: &mut Window,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        syntax: &Syntax,
        time: f32,
        dt: f32,
    ) {
        let mut keybind_handler = window.get_keybind_handler();

        while let Some(keybind) = keybind_handler.next(window) {
            match keybind {
                Keybind {
                    key: Key::O,
                    mods: MOD_CTRL,
                } => {
                    if let Ok(path) = find_file(FindFileKind::OpenFile) {
                        if let Some(doc_index) = self.open_or_reuse_doc(path.as_path(), line_pool) {
                            self.add_tab(doc_index);
                        }
                    }
                }
                Keybind {
                    key: Key::S,
                    mods: MOD_CTRL,
                } => {
                    if let Some((_, doc)) = self.get_focused_tab() {
                        Self::try_save_doc(doc);
                    }
                }
                Keybind {
                    key: Key::N,
                    mods: MOD_CTRL,
                } => {
                    let doc_index = self.add_doc(Doc::new(line_pool));
                    self.add_tab(doc_index);
                }
                Keybind {
                    key: Key::W,
                    mods: MOD_CTRL,
                } => {
                    self.close_tab();
                }
                Keybind {
                    key: Key::R,
                    mods: MOD_CTRL,
                } => {
                    if let Some((_, doc)) = self.get_focused_tab() {
                        Self::reload_doc(doc, line_pool);
                    }
                }
                Keybind {
                    key: Key::PageUp,
                    mods: MOD_CTRL,
                } => {
                    if self.focused_tab_index > 0 {
                        self.focused_tab_index -= 1;
                    }
                }
                Keybind {
                    key: Key::PageDown,
                    mods: MOD_CTRL,
                } => {
                    if self.focused_tab_index < self.tabs.len() - 1 {
                        self.focused_tab_index += 1;
                    }
                }
                _ => keybind_handler.unprocessed(window, keybind),
            }
        }

        if let Some((tab, doc)) = self.get_focused_tab() {
            tab.update(doc, window, line_pool, text_buffer, syntax, time, dt);
        }
    }

    fn get_focused_tab(&mut self) -> Option<(&mut Tab, &mut Doc)> {
        if let Some(tab) = self.tabs.get_mut(self.focused_tab_index) {
            if let Some(Some(doc)) = self.docs.get_mut(tab.doc_index()) {
                return Some((tab, doc));
            }
        }

        None
    }

    pub fn draw(&mut self, theme: &Theme, gfx: &mut Gfx) {
        if let Some((tab, doc)) = self.get_focused_tab() {
            tab.draw(doc, theme, gfx);
        }
    }

    pub fn confirm_close_docs(&mut self, reason: &str) {
        for doc in self.docs.iter_mut().filter_map(|doc| doc.as_mut()) {
            Self::confirm_close_doc(doc, reason, false);
        }
    }

    fn confirm_close_doc(doc: &mut Doc, reason: &str, is_cancelable: bool) -> bool {
        if doc.is_saved() {
            true
        } else {
            let text = format!(
                "{} has unsaved changes. Do you want to save it before {}?",
                doc.file_name(),
                reason
            );

            let message_kind = if is_cancelable {
                MessageKind::YesNoCancel
            } else {
                MessageKind::YesNo
            };

            match message("Unsaved Changes", &text, message_kind) {
                MessageResponse::Yes => Self::try_save_doc(doc),
                MessageResponse::No => true,
                MessageResponse::Cancel => false,
            }
        }
    }

    fn try_save_doc(doc: &mut Doc) -> bool {
        let path = if let Some(path) = doc.path() {
            Ok(path.to_owned())
        } else {
            find_file(FindFileKind::Save)
        };

        if let Err(err) = path.map(|path| doc.save(path)) {
            message("Failed to Save File", &err.to_string(), MessageKind::Ok);
            false
        } else {
            true
        }
    }

    fn reload_doc(doc: &mut Doc, line_pool: &mut LinePool) {
        if !Self::confirm_close_doc(doc, "reloading", true) {
            return;
        }

        let Some(path) = doc.path().map(|path| path.to_owned()) else {
            return;
        };

        if let Err(err) = doc.load(&path, line_pool) {
            message("Failed to Reload File", &err.to_string(), MessageKind::Ok);
        }
    }

    fn clamp_focused_tab(&mut self) {
        if self.focused_tab_index >= self.tabs.len() {
            if self.tabs.is_empty() {
                self.focused_tab_index = 0;
            } else {
                self.focused_tab_index = self.tabs.len() - 1;
            }
        }
    }

    fn add_doc(&mut self, doc: Doc) -> usize {
        let mut doc_index = None;

        for i in 0..self.docs.len() {
            if self.docs[i].is_none() {
                doc_index = Some(i);
            }
        }

        if let Some(doc_index) = doc_index {
            self.docs[doc_index] = Some(doc);
            doc_index
        } else {
            self.docs.push(Some(doc));
            self.docs.len() - 1
        }
    }

    fn add_tab(&mut self, doc_index: usize) {
        let tab = Tab::new(doc_index);

        if self.focused_tab_index >= self.tabs.len() {
            self.tabs.push(tab);
        } else {
            self.tabs.insert(self.focused_tab_index + 1, tab);
            self.focused_tab_index += 1;
        }
    }

    fn close_tab(&mut self) {
        let doc_index = if let Some(tab) = self.tabs.get(self.focused_tab_index) {
            tab.doc_index()
        } else {
            return;
        };

        let doc_usage_count = self
            .tabs
            .iter()
            .filter(|tab| tab.doc_index() == doc_index)
            .count();

        if doc_usage_count > 1 {
            self.tabs.remove(self.focused_tab_index);
            self.clamp_focused_tab();

            return;
        }

        if let Some(Some(doc)) = self.docs.get_mut(doc_index).as_mut() {
            if !Self::confirm_close_doc(doc, "closing", true) {
                return;
            }
        }

        self.docs[doc_index] = None;
        self.tabs.remove(self.focused_tab_index);
        self.clamp_focused_tab();
    }

    fn open_or_reuse_doc(&mut self, path: &Path, line_pool: &mut LinePool) -> Option<usize> {
        for (i, doc) in self.docs.iter().filter_map(|doc| doc.as_ref()).enumerate() {
            if doc.path() == Some(path) {
                return Some(i);
            }
        }

        let mut doc = Doc::new(line_pool);

        if let Err(err) = doc.load(path, line_pool) {
            message("Failed to Open File", &err.to_string(), MessageKind::Ok);
        } else {
            return Some(self.add_doc(doc));
        }

        None
    }
}
