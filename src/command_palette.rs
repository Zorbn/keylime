use std::{
    fs::read_dir,
    io,
    path::{Path, PathBuf},
};

use crate::{
    cursor_index::CursorIndex,
    dialog::{message, MessageKind},
    doc::{Doc, DocKind},
    editor::Editor,
    gfx::Gfx,
    key::Key,
    keybind::{Keybind, MOD_CTRL},
    line_pool::LinePool,
    mouse_button::MouseButton,
    mousebind::Mousebind,
    position::Position,
    rect::Rect,
    side::{SIDE_ALL, SIDE_LEFT, SIDE_RIGHT, SIDE_TOP},
    tab::Tab,
    temp_buffer::TempBuffer,
    theme::Theme,
    visual_position::VisualPosition,
    window::Window,
};

#[derive(PartialEq, Eq)]
enum CommandPaletteState {
    Inactive,
    Active,
}

const TITLE: &str = "Find File";

pub struct CommandPalette {
    state: CommandPaletteState,
    tab: Tab,
    doc: Doc,
    last_updated_version: Option<usize>,
    results: Vec<String>,
    selected_result_index: usize,

    bounds: Rect,
    title_bounds: Rect,
    input_bounds: Rect,
    result_bounds: Rect,
    results_bounds: Rect,
}

impl CommandPalette {
    pub fn new(line_pool: &mut LinePool) -> Self {
        Self {
            state: CommandPaletteState::Inactive,
            tab: Tab::new(0),
            doc: Doc::new(line_pool, DocKind::SingleLine),
            last_updated_version: None,
            results: Vec::new(),
            selected_result_index: 0,

            bounds: Rect::zero(),
            title_bounds: Rect::zero(),
            input_bounds: Rect::zero(),
            result_bounds: Rect::zero(),
            results_bounds: Rect::zero(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.state != CommandPaletteState::Inactive
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx) {
        let title_padding_x = gfx.glyph_width();
        let title_width =
            Gfx::measure_text(TITLE.chars()) as f32 * gfx.glyph_width() + title_padding_x * 2.0;

        self.title_bounds = Rect::new(0.0, 0.0, title_width, gfx.tab_height()).floor();

        self.input_bounds = Rect::new(0.0, 0.0, gfx.glyph_width() * 64.0, gfx.line_height() * 2.0)
            .below(self.title_bounds)
            .shift_y(-gfx.border_width())
            .floor();

        self.result_bounds = Rect::new(0.0, 0.0, self.input_bounds.width, gfx.line_height() * 1.25);

        self.results_bounds = Rect::new(
            0.0,
            0.0,
            self.input_bounds.width,
            self.result_bounds.height * self.results.len() as f32,
        )
        .below(self.input_bounds)
        .shift_y(-gfx.border_width())
        .floor();

        self.bounds = self
            .title_bounds
            .expand_to_include(self.input_bounds)
            .expand_to_include(self.results_bounds)
            .center_x_in(bounds)
            .offset_by(Rect::new(0.0, gfx.tab_height() * 2.0, 0.0, 0.0))
            .floor();

        self.tab.layout(
            Rect::zero(),
            Rect::new(0.0, 0.0, gfx.glyph_width() * 10.0, gfx.line_height())
                .center_in(self.input_bounds)
                .expand_width_in(self.input_bounds)
                .offset_by(self.bounds)
                .floor(),
        );
    }

    pub fn update(
        &mut self,
        editor: &mut Editor,
        window: &mut Window,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        time: f32,
        dt: f32,
    ) {
        if !self.is_active() {
            return;
        }

        let mut mouse_handler = window.get_mousebind_handler();

        while let Some(mousebind) = mouse_handler.next(window) {
            let position = VisualPosition::new(mousebind.x, mousebind.y);

            let Mousebind {
                button: None | Some(MouseButton::Left),
                ..
            } = mousebind
            else {
                continue;
            };

            if mousebind.button.is_some() && !self.bounds.contains_position(position) {
                self.close();
                continue;
            }

            let results_bounds = self.results_bounds.offset_by(self.bounds);

            if !results_bounds.contains_position(position) {
                continue;
            }

            let clicked_result_index =
                ((position.y - results_bounds.y) / self.result_bounds.height) as usize;

            if clicked_result_index >= self.results.len() {
                continue;
            }

            self.selected_result_index = clicked_result_index;

            if mousebind.button.is_some() {
                self.submit(editor, line_pool, time);
            }
        }

        let mut keybind_handler = window.get_keybind_handler();

        while let Some(keybind) = keybind_handler.next(window) {
            match keybind {
                Keybind {
                    key: Key::Escape,
                    mods: 0,
                } => {
                    self.close();
                }
                Keybind {
                    key: Key::Enter,
                    mods: 0,
                } => {
                    self.submit(editor, line_pool, time);
                }
                Keybind {
                    key: Key::Tab,
                    mods: 0,
                } => {
                    self.complete_result(line_pool, time);
                }
                Keybind {
                    key: Key::Up,
                    mods: 0,
                } => {
                    if self.selected_result_index > 0 {
                        self.selected_result_index -= 1;
                    }
                }
                Keybind {
                    key: Key::Down,
                    mods: 0,
                } => {
                    if self.selected_result_index < self.results.len() - 1 {
                        self.selected_result_index += 1;
                    }
                }
                Keybind {
                    key: Key::Backspace,
                    mods: 0 | MOD_CTRL,
                } => {
                    let cursor = self.doc.get_cursor(CursorIndex::Main);
                    let end = cursor.position;
                    let mut start = self.doc.move_position(end, Position::new(-1, 0));

                    if matches!(self.doc.get_char(start), '/' | '\\') {
                        start = self.find_path_component_start(start);

                        self.doc.delete(start, end, line_pool, time);
                    } else {
                        keybind_handler.unprocessed(window, keybind);
                    }
                }
                _ => keybind_handler.unprocessed(window, keybind),
            }
        }

        self.tab.update(
            &mut self.doc,
            window,
            line_pool,
            text_buffer,
            None,
            time,
            dt,
        );

        window.clear_inputs();

        if let Err(err) = self.update_results(line_pool, time) {
            message("Error Finding Files", &err.to_string(), MessageKind::Ok);
        }

        self.selected_result_index = self
            .selected_result_index
            .clamp(0, self.results.len().saturating_sub(1));
    }

    fn submit(&mut self, editor: &mut Editor, line_pool: &mut LinePool, time: f32) {
        self.complete_result(line_pool, time);

        if editor
            .open_file(Path::new(&self.doc.to_string()), line_pool)
            .is_ok()
        {
            self.close();
        }
    }

    fn complete_result(&mut self, line_pool: &mut LinePool, time: f32) {
        if let Some(result) = self.results.get(self.selected_result_index) {
            let line_len = self.doc.get_line_len(0);
            let end = Position::new(line_len, 0);
            let start = self.find_path_component_start(end);

            self.doc.delete(start, end, line_pool, time);

            let line_len = self.doc.get_line_len(0);
            let mut start = Position::new(line_len, 0);

            for c in result.chars() {
                self.doc.insert(start, &[c], line_pool, time);
                start = self.doc.move_position(start, Position::new(1, 0));
            }
        }
    }

    fn find_path_component_start(&self, position: Position) -> Position {
        let mut start = position;

        while start > Position::zero() {
            let next_start = self.doc.move_position(start, Position::new(-1, 0));

            if matches!(self.doc.get_char(next_start), '/' | '\\') {
                break;
            }

            start = next_start;
        }

        start
    }

    fn update_results(&mut self, line_pool: &mut LinePool, time: f32) -> io::Result<()> {
        if Some(self.doc.version()) == self.last_updated_version {
            return Ok(());
        }

        self.last_updated_version = Some(self.doc.version());

        let mut path = PathBuf::new();
        path.push(".");
        path.push(self.doc.to_string());

        let dir = if path.is_dir() {
            let line_len = self.doc.get_line_len(0);
            let last_char = self.doc.get_char(Position::new(line_len - 1, 0));

            if line_len > 0 && !matches!(last_char, '/' | '\\' | '.') {
                self.doc
                    .insert(Position::new(line_len, 0), &['/'], line_pool, time);
            }

            path.as_path()
        } else {
            path.parent().unwrap_or(Path::new("."))
        };

        self.results.clear();

        for entry in read_dir(dir)? {
            let entry = entry?;
            let entry_path = entry.path();

            if Self::does_path_match_prefix(&path, &entry_path) {
                if let Some(result) = entry_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|str| str.to_owned())
                {
                    self.results.push(result);
                }
            }
        }

        Ok(())
    }

    fn does_path_match_prefix(prefix: &Path, path: &Path) -> bool {
        for (prefix_component, path_component) in prefix.components().zip(path.components()) {
            let Some(prefix_component) = prefix_component.as_os_str().to_str() else {
                continue;
            };

            let Some(path_component) = path_component.as_os_str().to_str() else {
                continue;
            };

            for (prefix_char, path_char) in prefix_component.chars().zip(path_component.chars()) {
                if prefix_char.to_ascii_lowercase() != path_char.to_ascii_lowercase() {
                    return false;
                }
            }
        }

        true
    }

    pub fn draw(&mut self, theme: &Theme, gfx: &mut Gfx, is_focused: bool) {
        if !self.is_active() || self.last_updated_version.is_none() {
            return;
        }

        gfx.begin(Some(self.bounds));

        gfx.add_bordered_rect(
            self.results_bounds,
            SIDE_ALL,
            &theme.background,
            &theme.border,
        );

        gfx.add_bordered_rect(
            self.input_bounds,
            SIDE_ALL,
            &theme.background,
            &theme.border,
        );

        gfx.add_bordered_rect(
            self.title_bounds,
            SIDE_LEFT | SIDE_RIGHT | SIDE_TOP,
            &theme.background,
            &theme.border,
        );

        gfx.add_rect(
            self.title_bounds.top_border(gfx.border_width()),
            &theme.keyword,
        );

        gfx.add_text(
            TITLE.chars(),
            gfx.glyph_width(),
            gfx.border_width() + gfx.tab_padding_y() + gfx.border_width(),
            &theme.normal,
        );

        let doc_bounds = self.tab.doc_bounds();

        gfx.add_bordered_rect(
            doc_bounds
                .add_margin(gfx.border_width())
                .unoffset_by(self.bounds),
            SIDE_ALL,
            &theme.background,
            &theme.border,
        );

        gfx.end();

        self.tab.draw(&self.doc, theme, gfx, is_focused);

        gfx.begin(Some(self.results_bounds.offset_by(self.bounds)));

        for (i, result) in self.results.iter().enumerate() {
            let y = i as f32 * self.result_bounds.height
                + (self.result_bounds.height - gfx.glyph_height()) / 2.0;

            let color = if i == self.selected_result_index {
                &theme.keyword
            } else {
                &theme.normal
            };

            gfx.add_text(result.chars(), gfx.glyph_width(), y, color);
        }

        gfx.end();
    }

    pub fn open(&mut self) {
        self.state = CommandPaletteState::Active;
    }

    fn close(&mut self) {
        self.state = CommandPaletteState::Inactive;
    }
}
