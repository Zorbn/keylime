use std::{env::set_current_dir, io, path::Path};

use crate::{
    config::{theme::Theme, Config},
    geometry::{rect::Rect, visual_position::VisualPosition},
    input::{
        key::Key,
        keybind::{Keybind, MOD_CTRL, MOD_CTRL_SHIFT},
        mouse_button::MouseButton,
        mousebind::Mousebind,
    },
    platform::{
        dialog::{find_file, message, FindFileKind, MessageKind, MessageResponse},
        gfx::Gfx,
        window::Window,
    },
    temp_buffer::TempBuffer,
    text::{
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
    ui::{
        command_palette::{
            file_mode::MODE_OPEN_FILE,
            go_to_line_mode::MODE_GO_TO_LINE,
            search_mode::{MODE_SEARCH, MODE_SEARCH_AND_REPLACE_START},
            CommandPalette,
        },
        tab::Tab,
    },
};

pub struct Editor {
    docs: Vec<Option<Doc>>,
    tabs: Vec<Tab>,
    focused_tab_index: usize,
    bounds: Rect,
}

impl Editor {
    pub fn new(line_pool: &mut LinePool) -> Self {
        let mut editor = Self {
            docs: Vec::new(),
            tabs: Vec::new(),
            focused_tab_index: 0,
            bounds: Rect::zero(),
        };

        editor.add_doc(Doc::new(line_pool, DocKind::MultiLine));
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

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx) {
        self.bounds = bounds;

        let mut tab_x = 0.0;
        let tab_height = gfx.tab_height();

        for i in 0..self.tabs.len() {
            let Some((tab, doc)) = self.get_tab_with_doc(i) else {
                return;
            };

            let tab_width = gfx.glyph_width() * 4.0
                + Gfx::measure_text(doc.file_name().chars()) as f32 * gfx.glyph_width();

            let tab_bounds = Rect::new(tab_x, 0.0, tab_width, tab_height);
            let doc_bounds = bounds.shrink_top_by(tab_bounds);

            tab_x += tab_width - gfx.border_width();

            tab.layout(tab_bounds, doc_bounds, doc, gfx);
        }
    }

    pub fn update(
        &mut self,
        command_palette: &mut CommandPalette,
        window: &mut Window,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
        dt: f32,
    ) {
        let mut mousebind_handler = window.get_mousebind_handler();

        while let Some(mousebind) = mousebind_handler.next(window) {
            let visual_position =
                VisualPosition::new(mousebind.x - self.bounds.x, mousebind.y - self.bounds.y);

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: 0,
                    is_drag: false,
                    ..
                } => {
                    match self
                        .tabs
                        .iter()
                        .enumerate()
                        .filter(|(_, tab)| tab.tab_bounds().contains_position(visual_position))
                        .nth(0)
                    {
                        Some((i, _)) => self.focused_tab_index = i,
                        _ => mousebind_handler.unprocessed(window, mousebind),
                    }
                }
                _ => mousebind_handler.unprocessed(window, mousebind),
            }
        }

        let mut keybind_handler = window.get_keybind_handler();

        while let Some(keybind) = keybind_handler.next(window) {
            match keybind {
                Keybind {
                    key: Key::P,
                    mods: MOD_CTRL,
                } => {
                    command_palette.open(MODE_OPEN_FILE, self, line_pool, time);
                }
                Keybind {
                    key: Key::F,
                    mods: MOD_CTRL,
                } => {
                    command_palette.open(MODE_SEARCH, self, line_pool, time);
                }
                Keybind {
                    key: Key::H,
                    mods: MOD_CTRL,
                } => {
                    command_palette.open(MODE_SEARCH_AND_REPLACE_START, self, line_pool, time);
                }
                Keybind {
                    key: Key::G,
                    mods: MOD_CTRL,
                } => {
                    command_palette.open(MODE_GO_TO_LINE, self, line_pool, time);
                }
                Keybind {
                    key: Key::O,
                    mods: MOD_CTRL,
                } => {
                    if let Ok(path) = find_file(FindFileKind::OpenFile) {
                        if let Err(err) = self.open_file(path.as_path(), line_pool) {
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
                    if let Some((_, doc)) = self.get_tab_with_doc(self.focused_tab_index) {
                        Self::try_save_doc(doc, line_pool, time);
                    }
                }
                Keybind {
                    key: Key::N,
                    mods: MOD_CTRL,
                } => {
                    let doc_index = self.add_doc(Doc::new(line_pool, DocKind::MultiLine));
                    self.add_tab(doc_index);
                }
                Keybind {
                    key: Key::W,
                    mods: MOD_CTRL,
                } => {
                    self.close_tab(line_pool, time);
                }
                Keybind {
                    key: Key::R,
                    mods: MOD_CTRL,
                } => {
                    if let Some((_, doc)) = self.get_tab_with_doc(self.focused_tab_index) {
                        Self::reload_doc(doc, line_pool, time);
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

        if let Some((tab, doc)) = self.get_tab_with_doc(self.focused_tab_index) {
            let language = doc
                .path()
                .and_then(|path| path.extension())
                .and_then(|extension| extension.to_str())
                .and_then(|extension| config.get_language(extension));

            tab.update(doc, window, line_pool, text_buffer, language, time, dt);
        }

        window.clear_inputs();
    }

    pub fn draw(&mut self, theme: &Theme, gfx: &mut Gfx, is_focused: bool) {
        let tab_height = gfx.tab_height();

        gfx.begin(Some(self.bounds));

        for i in 0..self.tabs.len() {
            let Some((tab, doc)) = self.get_tab_with_doc(i) else {
                continue;
            };

            Self::draw_tab(tab, doc, theme, gfx);
        }

        let focused_tab_bounds = if let Some(tab) = self.tabs.get(self.focused_tab_index) {
            let tab_bounds = tab.tab_bounds();

            gfx.add_rect(tab_bounds.top_border(gfx.border_width()), &theme.keyword);

            tab_bounds
        } else {
            Rect::zero()
        };

        gfx.add_rect(
            Rect::from_sides(
                0.0,
                tab_height - gfx.border_width(),
                focused_tab_bounds.x,
                tab_height,
            ),
            &theme.border,
        );

        gfx.add_rect(
            Rect::from_sides(
                focused_tab_bounds.x + focused_tab_bounds.width,
                tab_height - gfx.border_width(),
                self.bounds.width,
                tab_height,
            ),
            &theme.border,
        );

        gfx.end();

        if let Some((tab, doc)) = self.get_tab_with_doc(self.focused_tab_index) {
            tab.draw(doc, theme, gfx, is_focused);
        }
    }

    fn draw_tab(tab: &Tab, doc: &Doc, theme: &Theme, gfx: &mut Gfx) {
        let tab_padding_y = gfx.tab_padding_y();
        let tab_bounds = tab.tab_bounds();

        gfx.add_rect(tab_bounds.left_border(gfx.border_width()), &theme.border);

        let text_x = (tab_bounds.x + gfx.glyph_width() * 2.0).floor();

        let text_width = gfx.add_text(
            doc.file_name().chars(),
            text_x,
            tab_padding_y,
            &theme.normal,
        );

        if !doc.is_saved() {
            gfx.add_text(
                "*".chars(),
                text_x + text_width as f32 * gfx.glyph_width(),
                tab_padding_y,
                &theme.symbol,
            );
        }

        gfx.add_rect(tab_bounds.right_border(gfx.border_width()), &theme.border);
    }

    pub fn get_tab_with_doc(&mut self, tab_index: usize) -> Option<(&mut Tab, &mut Doc)> {
        if let Some(tab) = self.tabs.get_mut(tab_index) {
            if let Some(Some(doc)) = self.docs.get_mut(tab.doc_index()) {
                return Some((tab, doc));
            }
        }

        None
    }

    pub fn focused_tab_index(&self) -> usize {
        self.focused_tab_index
    }

    pub fn open_file(&mut self, path: &Path, line_pool: &mut LinePool) -> io::Result<()> {
        let doc_index = self.open_or_reuse_doc(path, line_pool)?;

        for tab in &self.tabs {
            if tab.doc_index() == doc_index {
                self.focused_tab_index = doc_index;

                return Ok(());
            }
        }

        self.add_tab(doc_index);

        Ok(())
    }

    pub fn confirm_close_docs(&mut self, reason: &str, line_pool: &mut LinePool, time: f32) {
        for doc in self.docs.iter_mut().filter_map(|doc| doc.as_mut()) {
            Self::confirm_close_doc(doc, reason, false, line_pool, time);
        }
    }

    fn confirm_close_doc(
        doc: &mut Doc,
        reason: &str,
        is_cancelable: bool,
        line_pool: &mut LinePool,
        time: f32,
    ) -> bool {
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
                MessageResponse::Yes => Self::try_save_doc(doc, line_pool, time),
                MessageResponse::No => true,
                MessageResponse::Cancel => false,
            }
        }
    }

    fn try_save_doc(doc: &mut Doc, line_pool: &mut LinePool, time: f32) -> bool {
        let path = if let Some(path) = doc.path() {
            Ok(path.to_owned())
        } else {
            find_file(FindFileKind::Save)
        };

        let Ok(path) = path else {
            return false;
        };

        doc.trim_trailing_whitespace(line_pool, time);

        if let Err(err) = doc.save(path) {
            message("Failed to Save File", &err.to_string(), MessageKind::Ok);
            false
        } else {
            true
        }
    }

    fn reload_doc(doc: &mut Doc, line_pool: &mut LinePool, time: f32) {
        if !Self::confirm_close_doc(doc, "reloading", true, line_pool, time) {
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
                break;
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

    fn close_tab(&mut self, line_pool: &mut LinePool, time: f32) {
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
            if !Self::confirm_close_doc(doc, "closing", true, line_pool, time) {
                return;
            }
        }

        self.docs[doc_index] = None;
        self.tabs.remove(self.focused_tab_index);
        self.clamp_focused_tab();
    }

    fn open_or_reuse_doc(&mut self, path: &Path, line_pool: &mut LinePool) -> io::Result<usize> {
        for (i, doc) in self.docs.iter().filter_map(|doc| doc.as_ref()).enumerate() {
            if doc.path() == Some(path) {
                return Ok(i);
            }
        }

        let mut doc = Doc::new(line_pool, DocKind::MultiLine);

        doc.load(path, line_pool)?;

        Ok(self.add_doc(doc))
    }
}