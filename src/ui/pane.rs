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
        dialog::{find_file, message, FindFileKind, MessageKind},
        gfx::Gfx,
        window::Window,
    },
    temp_buffer::TempBuffer,
    text::{
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
};

use super::{
    command_palette::{
        file_mode::MODE_OPEN_FILE,
        go_to_line_mode::MODE_GO_TO_LINE,
        search_mode::{MODE_SEARCH, MODE_SEARCH_AND_REPLACE_START},
        CommandPalette,
    },
    doc_list::DocList,
    tab::Tab,
};

pub struct Pane {
    tabs: Vec<Tab>,
    focused_tab_index: usize,
    bounds: Rect,
}

impl Pane {
    pub fn new(
        doc_list: &mut DocList,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) -> Self {
        let mut pane = Self {
            tabs: Vec::new(),
            focused_tab_index: 0,
            bounds: Rect::zero(),
        };

        let doc_index = doc_list.add(Doc::new(line_pool, DocKind::MultiLine));
        pane.add_tab(doc_index, doc_list, config, line_pool, time);

        pane
    }

    pub fn is_animating(&self) -> bool {
        if let Some(tab) = self.tabs.get(self.focused_tab_index) {
            tab.is_animating()
        } else {
            false
        }
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx, doc_list: &mut DocList) {
        self.bounds = bounds;

        let mut tab_x = 0.0;
        let tab_height = gfx.tab_height();

        for i in 0..self.tabs.len() {
            let Some((tab, doc)) = self.get_tab_with_doc_mut(i, doc_list) else {
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
        doc_list: &mut DocList,
        command_palette: &mut CommandPalette,
        window: &mut Window,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
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
                    command_palette.open(MODE_OPEN_FILE, self, doc_list, config, line_pool, time);
                }
                Keybind {
                    key: Key::F,
                    mods: MOD_CTRL,
                } => {
                    command_palette.open(MODE_SEARCH, self, doc_list, config, line_pool, time);
                }
                Keybind {
                    key: Key::H,
                    mods: MOD_CTRL,
                } => {
                    command_palette.open(
                        MODE_SEARCH_AND_REPLACE_START,
                        self,
                        doc_list,
                        config,
                        line_pool,
                        time,
                    );
                }
                Keybind {
                    key: Key::G,
                    mods: MOD_CTRL,
                } => {
                    command_palette.open(MODE_GO_TO_LINE, self, doc_list, config, line_pool, time);
                }
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
                    if let Some((_, doc)) =
                        self.get_tab_with_doc_mut(self.focused_tab_index, doc_list)
                    {
                        DocList::try_save(doc, config, line_pool, time);
                    }
                }
                Keybind {
                    key: Key::N,
                    mods: MOD_CTRL,
                } => {
                    let doc_index = doc_list.add(Doc::new(line_pool, DocKind::MultiLine));
                    self.add_tab(doc_index, doc_list, config, line_pool, time);
                }
                Keybind {
                    key: Key::W,
                    mods: MOD_CTRL,
                } => {
                    self.close_tab(doc_list, config, line_pool, time);
                }
                Keybind {
                    key: Key::R,
                    mods: MOD_CTRL,
                } => {
                    if let Some((tab, doc)) =
                        self.get_tab_with_doc_mut(self.focused_tab_index, doc_list)
                    {
                        DocList::reload(doc, config, line_pool, time);
                        tab.camera.recenter();
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

        if let Some((tab, doc)) = self.get_tab_with_doc_mut(self.focused_tab_index, doc_list) {
            tab.update(doc, window, line_pool, text_buffer, config, time);
        }
    }

    pub fn update_camera(&mut self, doc_list: &mut DocList, window: &mut Window, dt: f32) {
        if let Some((tab, doc)) = self.get_tab_with_doc_mut(self.focused_tab_index, doc_list) {
            tab.update_camera(doc, window, dt);
        }
    }

    pub fn draw(
        &mut self,
        doc_list: &mut DocList,
        config: &Config,
        gfx: &mut Gfx,
        is_focused: bool,
    ) {
        let tab_height = gfx.tab_height();

        gfx.begin(Some(self.bounds));

        gfx.add_rect(
            self.bounds
                .left_border(gfx.border_width())
                .unoffset_by(self.bounds),
            &config.theme.border,
        );

        for i in 0..self.tabs.len() {
            let Some((tab, doc)) = self.get_tab_with_doc(i, doc_list) else {
                continue;
            };

            Self::draw_tab(tab, doc, &config.theme, gfx);
        }

        let focused_tab_bounds = if let Some(tab) = is_focused
            .then(|| self.tabs.get(self.focused_tab_index))
            .flatten()
        {
            let tab_bounds = tab.tab_bounds();

            gfx.add_rect(
                tab_bounds.top_border(gfx.border_width()),
                &config.theme.keyword,
            );

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
            &config.theme.border,
        );

        gfx.add_rect(
            Rect::from_sides(
                focused_tab_bounds.x + focused_tab_bounds.width,
                tab_height - gfx.border_width(),
                self.bounds.width,
                tab_height,
            ),
            &config.theme.border,
        );

        gfx.end();

        if let Some((tab, doc)) = self.get_tab_with_doc_mut(self.focused_tab_index, doc_list) {
            tab.draw(doc, config, gfx, is_focused);
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

    pub fn get_tab_with_doc_mut<'a>(
        &'a mut self,
        tab_index: usize,
        doc_list: &'a mut DocList,
    ) -> Option<(&'a mut Tab, &'a mut Doc)> {
        if let Some(tab) = self.tabs.get_mut(tab_index) {
            if let Some(doc) = doc_list.get_mut(tab.doc_index()) {
                return Some((tab, doc));
            }
        }

        None
    }

    pub fn get_tab_with_doc<'a>(
        &'a self,
        tab_index: usize,
        doc_list: &'a DocList,
    ) -> Option<(&'a Tab, &'a Doc)> {
        if let Some(tab) = self.tabs.get(tab_index) {
            if let Some(doc) = doc_list.get(tab.doc_index()) {
                return Some((tab, doc));
            }
        }

        None
    }

    pub fn open_file(
        &mut self,
        path: &Path,
        doc_list: &mut DocList,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) -> io::Result<()> {
        let doc_index = doc_list.open_or_reuse(path, line_pool)?;

        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.doc_index() == doc_index {
                self.focused_tab_index = i;

                return Ok(());
            }
        }

        self.add_tab(doc_index, doc_list, config, line_pool, time);

        Ok(())
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

    fn add_tab(
        &mut self,
        doc_index: usize,
        doc_list: &mut DocList,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let is_doc_worthless = doc_list
            .get(doc_index)
            .map(|doc| doc.is_worthless())
            .unwrap_or(false);

        if let Some((_, doc)) = self.get_tab_with_doc(self.focused_tab_index, doc_list) {
            let is_focused_doc_worthless = doc.is_worthless();

            if !is_doc_worthless && is_focused_doc_worthless {
                self.close_tab(doc_list, config, line_pool, time);
            }
        }

        let tab = Tab::new(doc_index);

        doc_list
            .get_mut(doc_index)
            .expect("tried to add a tab referencing a non-existent doc")
            .add_usage();

        if self.focused_tab_index >= self.tabs.len() {
            self.tabs.push(tab);
        } else {
            self.tabs.insert(self.focused_tab_index + 1, tab);
            self.focused_tab_index += 1;
        }
    }

    fn close_tab(
        &mut self,
        doc_list: &mut DocList,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) -> bool {
        let doc_index = if let Some(tab) = self.tabs.get(self.focused_tab_index) {
            tab.doc_index()
        } else {
            return true;
        };

        let Some(doc) = doc_list.get_mut(doc_index) else {
            return true;
        };

        if doc.usages() > 1 {
            doc.remove_usage();

            self.tabs.remove(self.focused_tab_index);
            self.clamp_focused_tab();

            return true;
        }

        if !DocList::confirm_close(doc, "closing", true, config, line_pool, time) {
            return false;
        }

        doc_list.remove(doc_index, line_pool);
        self.tabs.remove(self.focused_tab_index);
        self.clamp_focused_tab();

        true
    }

    pub fn close_all_tabs(
        &mut self,
        doc_list: &mut DocList,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) -> bool {
        while !self.tabs.is_empty() {
            if !self.close_tab(doc_list, config, line_pool, time) {
                return false;
            }
        }

        true
    }

    pub fn tabs_len(&self) -> usize {
        self.tabs.len()
    }

    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn focused_tab_index(&self) -> usize {
        self.focused_tab_index
    }
}
