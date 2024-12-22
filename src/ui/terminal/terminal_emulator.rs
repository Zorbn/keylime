use std::{iter, mem::swap, ops::RangeInclusive};

use crate::{
    config::Config,
    geometry::{position::Position, rect::Rect},
    input::{
        editing_actions::handle_copy,
        key::Key,
        keybind::{Keybind, MOD_ALT, MOD_CTRL, MOD_SHIFT},
    },
    platform::pty::Pty,
    temp_buffer::TempBuffer,
    text::{
        cursor::Cursor, doc::Doc, line_pool::LinePool, syntax_highlighter::TerminalHighlightKind,
    },
    ui::{camera::CameraRecenterKind, tab::Tab, widget::Widget, UiHandle},
};

use super::char32::*;

const MAX_SCROLLBACK_LINES: usize = 100;
const MIN_GRID_WIDTH: isize = 1;
const MIN_GRID_HEIGHT: isize = 1;

#[cfg(target_os = "windows")]
const SHELLS: &[&str] = &[
    "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
    "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
    "C:\\Windows\\system32\\cmd.exe",
];

#[cfg(target_os = "macos")]
const SHELLS: &[&str] = &["zsh", "bash", "sh"];

type GridLineColors = Vec<Vec<(TerminalHighlightKind, TerminalHighlightKind)>>;

pub struct TerminalEmulator {
    pty: Option<Pty>,

    // The position of the terminal's cursor, which follows different rules
    // compared to the document's cursor for compatibility reasons, and may be
    // different from the document's cursor position is the user is selecting text.
    pub grid_cursor: Position,
    pub grid_width: isize,
    pub grid_height: isize,
    pub grid_line_colors: GridLineColors,

    maintain_cursor_positions: bool,

    // Data for either the normal buffer or the alternate buffer,
    // depending on which one isn't currently being used.
    pub saved_grid_cursor: Position,
    pub saved_grid_line_colors: GridLineColors,

    saved_maintain_cursor_positions: bool,

    pub is_cursor_visible: bool,
    pub foreground_color: TerminalHighlightKind,
    pub background_color: TerminalHighlightKind,
    pub are_colors_swapped: bool,
    pub are_colors_bright: bool,
    pub scroll_top: isize,
    pub scroll_bottom: isize,
    pub is_in_alternate_buffer: bool,
}

impl TerminalEmulator {
    pub fn new() -> Self {
        let grid_width = MIN_GRID_WIDTH;
        let grid_height = MIN_GRID_HEIGHT;

        let mut emulator = Self {
            pty: Pty::new(grid_width, grid_height, SHELLS).ok(),

            grid_cursor: Position::zero(),
            grid_width,
            grid_height,
            grid_line_colors: Vec::new(),

            maintain_cursor_positions: false,

            saved_grid_cursor: Position::zero(),
            saved_grid_line_colors: Vec::new(),

            saved_maintain_cursor_positions: false,

            is_cursor_visible: true,
            foreground_color: TerminalHighlightKind::Foreground,
            background_color: TerminalHighlightKind::Background,
            are_colors_swapped: false,
            are_colors_bright: false,
            scroll_top: 0,
            scroll_bottom: grid_height - 1,
            is_in_alternate_buffer: false,
        };

        emulator.resize_grid_line_colors();

        emulator
    }

    pub fn update_input(
        &mut self,
        widget: &Widget,
        ui: &mut UiHandle,
        docs: &mut (Doc, Doc),
        tab: &mut Tab,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
    ) {
        let Some(mut pty) = self.pty.take() else {
            return;
        };

        let doc = self.get_doc_mut(docs);

        let mut keybind_handler = widget.get_keybind_handler(ui);

        while let Some(keybind) = keybind_handler.next(ui.window) {
            match keybind {
                Keybind {
                    key: Key::Enter, ..
                } => {
                    pty.input().push('\r' as u32);
                }
                Keybind {
                    key: Key::Escape, ..
                } => {
                    pty.input().push(0x1B);
                }
                Keybind { key: Key::Tab, .. } => {
                    pty.input().push('\t' as u32);
                }
                Keybind {
                    key: Key::Backspace,
                    mods,
                } => {
                    let key_char = if mods & MOD_CTRL != 0 { 0x8 } else { 0x7F };

                    pty.input().extend_from_slice(&[key_char]);
                }
                Keybind {
                    key: Key::Up | Key::Down | Key::Left | Key::Right | Key::Home | Key::End,
                    mods,
                } => {
                    let key_char = match keybind.key {
                        Key::Up => 'A',
                        Key::Down => 'B',
                        Key::Left => 'D',
                        Key::Right => 'C',
                        Key::Home => 'H',
                        Key::End => 'F',
                        _ => unreachable!(),
                    };

                    pty.input().extend_from_slice(&[0x1B, LEFT_BRACKET]);

                    if mods != 0 {
                        pty.input().extend_from_slice(&[ONE, SEMICOLON]);
                    }

                    if mods & MOD_SHIFT != 0 && mods & MOD_CTRL != 0 {
                        pty.input().push(SIX);
                    } else if mods & MOD_SHIFT != 0 && mods & MOD_ALT != 0 {
                        pty.input().push(FOUR);
                    } else if mods & MOD_SHIFT != 0 {
                        pty.input().push(TWO);
                    } else if mods & MOD_CTRL != 0 {
                        pty.input().push(FIVE);
                    } else if mods & MOD_ALT != 0 {
                        pty.input().push(THREE);
                    }

                    pty.input().push(key_char as u32);
                }
                Keybind {
                    key: Key::C | Key::X,
                    mods: MOD_CTRL,
                } => {
                    let mut has_selection = false;

                    for index in doc.cursor_indices() {
                        if doc.get_cursor(index).get_selection().is_some() {
                            has_selection = true;
                            break;
                        }
                    }

                    if has_selection {
                        handle_copy(ui.window, doc, text_buffer);
                    } else {
                        pty.input().push(keybind.key as u32 & 0x1F);
                    }
                }
                Keybind {
                    key: Key::V,
                    mods: MOD_CTRL,
                } => {
                    let text = ui.window.get_clipboard().unwrap_or(&[]);

                    for c in text {
                        pty.input().push(*c as u32);
                    }
                }
                Keybind {
                    key,
                    mods: MOD_CTRL,
                } => {
                    const KEY_A: u32 = Key::A as u32;
                    const KEY_Z: u32 = Key::Z as u32;

                    let key = key as u32;

                    if matches!(key, KEY_A..=KEY_Z) {
                        pty.input().push(key & 0x1F);
                    }
                }
                _ => {}
            }
        }

        let mut char_handler = widget.get_char_handler(ui);

        while let Some(c) = char_handler.next(ui.window) {
            pty.input().push(c as u32);
        }

        pty.flush();

        self.pty = Some(pty);

        tab.update(widget, ui, doc, line_pool, text_buffer, config, time);
    }

    pub fn update_output(
        &mut self,
        ui: &mut UiHandle,
        docs: &mut (Doc, Doc),
        tab: &mut Tab,
        line_pool: &mut LinePool,
        cursor_buffer: &mut TempBuffer<Cursor>,
        config: &Config,
        time: f32,
        dt: f32,
    ) {
        let Some(mut pty) = self.pty.take() else {
            return;
        };

        self.resize_grid(ui, tab, &mut pty);

        let cursor_buffer = cursor_buffer.get_mut();

        self.maintain_cursor_positions = true;

        let doc = self.get_doc_mut(docs);
        self.backup_doc_cursor_positions(doc, cursor_buffer);
        self.expand_doc_to_grid_size(doc, line_pool, time);

        let (input, output) = pty.input_output();

        if let Ok(mut output) = output.try_lock() {
            self.handle_escape_sequences(docs, input, &output, line_pool, &config.theme, time);

            output.clear();
        }

        let doc = self.get_doc_mut(docs);
        self.trim_excess_lines(ui, tab, doc, line_pool, time);

        if self.maintain_cursor_positions {
            self.restore_doc_cursor_positions(doc, cursor_buffer);
        }

        self.pty = Some(pty);

        let doc = self.get_doc(docs);

        tab.camera.horizontal.reset_velocity();
        tab.update_camera(ui, doc, dt);
    }

    fn resize_grid(&mut self, ui: &mut UiHandle, tab: &Tab, pty: &mut Pty) {
        let Rect {
            width: doc_width,
            height: doc_height,
            ..
        } = tab.doc_bounds();

        let grid_width = (doc_width / ui.gfx().glyph_width()).floor() as isize;
        let grid_width = grid_width.max(MIN_GRID_WIDTH);

        let grid_height = (doc_height / ui.gfx().line_height()).floor() as isize;
        let grid_height = grid_height.max(MIN_GRID_HEIGHT);

        if grid_width != self.grid_width || grid_height != self.grid_height {
            pty.resize(grid_width, grid_height);

            self.grid_width = grid_width;
            self.grid_height = grid_height;

            self.scroll_top = 0;
            self.scroll_bottom = grid_height - 1;

            self.resize_grid_line_colors();
        }
    }

    fn resize_grid_line_colors(&mut self) {
        Self::resize_grid_line_colors_for_buffer(
            &mut self.grid_line_colors,
            self.grid_width,
            self.grid_height,
        );
        Self::resize_grid_line_colors_for_buffer(
            &mut self.saved_grid_line_colors,
            self.grid_width,
            self.grid_height,
        );
    }

    fn resize_grid_line_colors_for_buffer(
        grid_line_colors: &mut GridLineColors,
        grid_width: isize,
        grid_height: isize,
    ) {
        grid_line_colors.resize(
            grid_height as usize,
            Vec::with_capacity(grid_width as usize),
        );

        for y in 0..grid_height {
            grid_line_colors[y as usize].resize(
                grid_width as usize,
                (
                    TerminalHighlightKind::Foreground,
                    TerminalHighlightKind::Background,
                ),
            );
        }
    }

    pub fn trim_excess_lines(
        &mut self,
        ui: &mut UiHandle,
        tab: &mut Tab,
        doc: &mut Doc,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let max_lines = self.grid_height as usize + MAX_SCROLLBACK_LINES;

        if doc.lines().len() <= max_lines {
            return;
        }

        let gfx = ui.gfx();

        let camera_offset_y =
            tab.camera.vertical.position - doc.lines().len() as f32 * gfx.line_height();

        let excess_lines = doc.lines().len() - max_lines;

        let start = Position::zero();
        let end = Position::new(0, excess_lines as isize);

        doc.delete(start, end, line_pool, time);
        doc.recycle_highlighted_lines_up_to_y(excess_lines);

        tab.camera.vertical.position =
            doc.lines().len() as f32 * gfx.line_height() + camera_offset_y;
    }

    // Scrolls the text in the region down, giving the impression that the camera is panning up.
    pub fn scroll_grid_region_down(
        &mut self,
        region: RangeInclusive<isize>,
        doc: &mut Doc,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let scroll_top = *region.start();
        let scroll_bottom = *region.end();

        let delete_start = self
            .grid_position_to_doc_position(Position::new(self.grid_width, scroll_bottom - 1), doc);

        let delete_end =
            self.grid_position_to_doc_position(Position::new(self.grid_width, scroll_bottom), doc);

        let insert_start = self.grid_position_to_doc_position(Position::new(0, scroll_top), doc);

        doc.delete(delete_start, delete_end, line_pool, time);

        let new_line_chars = iter::repeat(' ')
            .take(self.grid_width as usize)
            .chain("\n".chars());

        doc.insert(insert_start, new_line_chars, line_pool, time);

        let bottom_grid_line = self.grid_line_colors.remove(scroll_bottom as usize);
        self.grid_line_colors
            .insert(scroll_top as usize, bottom_grid_line);

        self.delete(
            Position::new(0, scroll_top),
            Position::new(self.grid_width, scroll_top),
            doc,
            line_pool,
            time,
        );

        self.highlight_lines(region, doc);
    }

    // Scrolls the text in the region up, giving the impression that the camera is panning down.
    pub fn scroll_grid_region_up(
        &mut self,
        region: RangeInclusive<isize>,
        doc: &mut Doc,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let scroll_top = *region.start();
        let scroll_bottom = *region.end();

        let should_use_scrollback = scroll_top == 0 && !self.is_in_alternate_buffer;

        let insert_start = if should_use_scrollback {
            self.grid_position_to_doc_position(Position::new(self.grid_width, scroll_bottom), doc)
        } else {
            // We need to delete the line that got scrolled out:
            let delete_start =
                self.grid_position_to_doc_position(Position::new(0, scroll_top), doc);
            let delete_end = Position::new(0, delete_start.y + 1);

            let insert_start = self.grid_position_to_doc_position(
                Position::new(self.grid_width, scroll_bottom - 1),
                doc,
            );

            doc.delete(delete_start, delete_end, line_pool, time);

            insert_start
        };

        let new_line_chars = "\n"
            .chars()
            .chain(iter::repeat(' ').take(self.grid_width as usize));

        doc.insert(insert_start, new_line_chars, line_pool, time);

        let top_grid_line = self.grid_line_colors.remove(scroll_top as usize);
        self.grid_line_colors
            .insert(scroll_bottom as usize, top_grid_line);

        self.delete(
            Position::new(0, scroll_bottom),
            Position::new(self.grid_width, scroll_bottom),
            doc,
            line_pool,
            time,
        );

        self.highlight_lines(region, doc);
    }

    pub fn switch_to_alternate_buffer(&mut self) {
        if self.is_in_alternate_buffer {
            return;
        }

        self.switch_buffer();
    }

    pub fn switch_to_normal_buffer(&mut self) {
        if !self.is_in_alternate_buffer {
            return;
        }

        self.switch_buffer();
    }

    pub fn switch_buffer(&mut self) {
        swap(&mut self.grid_cursor, &mut self.saved_grid_cursor);
        swap(&mut self.grid_line_colors, &mut self.saved_grid_line_colors);
        swap(
            &mut self.maintain_cursor_positions,
            &mut self.saved_maintain_cursor_positions,
        );

        self.is_in_alternate_buffer = !self.is_in_alternate_buffer;
    }

    fn expand_doc_to_grid_size(&mut self, doc: &mut Doc, line_pool: &mut LinePool, time: f32) {
        while (doc.lines().len() as isize) < self.grid_height {
            let start = doc.end();
            doc.insert(start, ['\n'], line_pool, time);
        }

        for y in 0..self.grid_height {
            let doc_y = doc.lines().len() as isize - self.grid_height + y;

            if doc.get_line_len(doc_y) >= self.grid_width {
                continue;
            }

            while doc.get_line_len(doc_y) < self.grid_width {
                let start = Position::new(doc.get_line_len(doc_y), doc_y);
                doc.insert(start, [' '], line_pool, time);
            }

            doc.highlight_line_from_terminal_colors(
                &self.grid_line_colors[y as usize],
                doc_y as usize,
            );
        }
    }

    fn clamp_position(&self, position: Position) -> Position {
        Position::new(
            position.x.clamp(0, self.grid_width - 1),
            position.y.clamp(0, self.grid_height - 1),
        )
    }

    pub fn move_position(&self, position: Position, delta: Position) -> Position {
        self.clamp_position(Position::new(position.x + delta.x, position.y + delta.y))
    }

    pub fn grid_position_to_doc_position(&self, position: Position, doc: &Doc) -> Position {
        Position::new(
            position.x,
            doc.lines().len() as isize - self.grid_height + position.y,
        )
    }

    fn doc_position_to_grid_position(&self, position: Position, doc: &Doc) -> Position {
        Position::new(
            position.x,
            position.y - (doc.lines().len() as isize - self.grid_height).max(0),
        )
    }

    fn backup_doc_cursor_positions(&mut self, doc: &Doc, cursor_buffer: &mut Vec<Cursor>) {
        doc.backup_cursors(cursor_buffer);
        self.convert_cursor_backups(doc, cursor_buffer, Self::doc_position_to_grid_position);
    }

    fn restore_doc_cursor_positions(&mut self, doc: &mut Doc, cursor_buffer: &mut [Cursor]) {
        self.convert_cursor_backups(doc, cursor_buffer, Self::grid_position_to_doc_position);
        doc.restore_cursors(cursor_buffer);
    }

    fn convert_cursor_backups(
        &mut self,
        doc: &Doc,
        cursor_buffer: &mut [Cursor],
        convert_fn: fn(&Self, Position, &Doc) -> Position,
    ) {
        for i in 0..cursor_buffer.len() {
            let cursor = &cursor_buffer[i];

            let position = convert_fn(self, cursor.position, doc);

            let selection_anchor = cursor
                .selection_anchor
                .map(|selection_anchor| convert_fn(self, selection_anchor, doc));

            cursor_buffer[i].position = position;
            cursor_buffer[i].selection_anchor = selection_anchor;
        }
    }

    pub fn jump_doc_cursors_to_grid_cursor(&mut self, doc: &mut Doc) {
        if !self.is_cursor_visible {
            return;
        }

        self.maintain_cursor_positions = false;

        let doc_position =
            self.grid_position_to_doc_position(self.clamp_position(self.grid_cursor), doc);
        doc.jump_cursors(doc_position, false);
    }

    pub fn move_cursor(&mut self, delta: Position, doc: &mut Doc) {
        self.jump_cursor(
            Position::new(self.grid_cursor.x + delta.x, self.grid_cursor.y + delta.y),
            doc,
        );
    }

    pub fn jump_cursor(&mut self, position: Position, doc: &mut Doc) {
        self.grid_cursor = self.clamp_position(position);

        self.jump_doc_cursors_to_grid_cursor(doc);
    }

    pub fn insert_at_cursor(
        &mut self,
        text: &[char],
        doc: &mut Doc,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        for c in text {
            if self.grid_cursor.x >= self.grid_width {
                self.jump_cursor(Position::new(0, self.grid_cursor.y + 1), doc);
            }

            self.insert(self.grid_cursor, &[*c], doc, line_pool, time);

            self.grid_cursor.x += 1;
            self.jump_doc_cursors_to_grid_cursor(doc);
        }
    }

    fn color_to_bright(color: TerminalHighlightKind) -> TerminalHighlightKind {
        match color {
            TerminalHighlightKind::Foreground => TerminalHighlightKind::BrightForeground,
            TerminalHighlightKind::Background => TerminalHighlightKind::BrightBackground,
            TerminalHighlightKind::Red => TerminalHighlightKind::BrightRed,
            TerminalHighlightKind::Green => TerminalHighlightKind::BrightGreen,
            TerminalHighlightKind::Yellow => TerminalHighlightKind::BrightYellow,
            TerminalHighlightKind::Blue => TerminalHighlightKind::BrightBlue,
            TerminalHighlightKind::Magenta => TerminalHighlightKind::BrightMagenta,
            TerminalHighlightKind::Cyan => TerminalHighlightKind::BrightCyan,
            _ => color,
        }
    }

    pub fn highlight_lines(&mut self, range: RangeInclusive<isize>, doc: &mut Doc) {
        for y in range {
            let doc_position = self.grid_position_to_doc_position(Position::new(0, y), doc);

            doc.highlight_line_from_terminal_colors(
                &self.grid_line_colors[y as usize],
                doc_position.y as usize,
            );
        }
    }

    pub fn insert(
        &mut self,
        start: Position,
        text: &[char],
        doc: &mut Doc,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let mut position = start;

        let colors = if self.are_colors_swapped {
            (self.background_color, self.foreground_color)
        } else {
            (self.foreground_color, self.background_color)
        };

        let colors = if self.are_colors_bright {
            (Self::color_to_bright(colors.0), colors.1)
        } else {
            colors
        };

        for c in text {
            let next_position = self.move_position(position, Position::new(1, 0));

            {
                let position = self.grid_position_to_doc_position(position, doc);
                let next_position = self.grid_position_to_doc_position(next_position, doc);

                doc.delete(position, next_position, line_pool, time);
                doc.insert(position, [*c], line_pool, time);
            }

            self.grid_line_colors[position.y as usize][position.x as usize] = colors;
            position = next_position;
        }

        self.jump_doc_cursors_to_grid_cursor(doc);

        let doc_start = self.grid_position_to_doc_position(start, doc);

        doc.highlight_line_from_terminal_colors(
            &self.grid_line_colors[start.y as usize],
            doc_start.y as usize,
        );
    }

    pub fn delete(
        &mut self,
        start: Position,
        end: Position,
        doc: &mut Doc,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        for y in start.y..=end.y {
            let start_x = if y == start.y { start.x } else { 0 };
            let end_x = if y == end.y { end.x } else { self.grid_width };

            for x in start_x..end_x {
                self.insert(Position::new(x, y), &[' '], doc, line_pool, time);
            }
        }
    }

    pub fn get_doc<'a>(&self, (normal_doc, alternate_doc): &'a (Doc, Doc)) -> &'a Doc {
        if self.is_in_alternate_buffer {
            alternate_doc
        } else {
            normal_doc
        }
    }

    pub fn get_doc_mut<'a>(&self, (normal_doc, alternate_doc): &'a mut (Doc, Doc)) -> &'a mut Doc {
        if self.is_in_alternate_buffer {
            alternate_doc
        } else {
            normal_doc
        }
    }

    pub fn pty(&mut self) -> Option<&mut Pty> {
        self.pty.as_mut()
    }
}
