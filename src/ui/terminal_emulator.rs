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
    text::{doc::Doc, line_pool::LinePool, selection::Selection},
};

use super::{camera::CameraRecenterKind, color::Color, tab::Tab, widget::Widget, UiHandle};

const ZERO: u32 = '0' as u32;
const ONE: u32 = '1' as u32;
const TWO: u32 = '2' as u32;
const THREE: u32 = '3' as u32;
const FOUR: u32 = '4' as u32;
const FIVE: u32 = '5' as u32;
const SIX: u32 = '6' as u32;
const EIGHT: u32 = '8' as u32;
const NINE: u32 = '9' as u32;
const SEMICOLON: u32 = ';' as u32;
const QUESTION_MARK: u32 = '?' as u32;
const LEFT_BRACKET: u32 = '[' as u32;
const RIGHT_BRACKET: u32 = ']' as u32;
const BACK_SLASH: u32 = '\\' as u32;
const LOWERCASE_L: u32 = 'l' as u32;
const LOWERCASE_H: u32 = 'h' as u32;
const LOWERCASE_M: u32 = 'm' as u32;
const UPPERCASE_C: u32 = 'C' as u32;
const UPPERCASE_H: u32 = 'H' as u32;
const UPPERCASE_J: u32 = 'J' as u32;
const UPPERCASE_K: u32 = 'K' as u32;
const UPPERCASE_X: u32 = 'X' as u32;

const MAX_SCROLLBACK_LINES: usize = 100;
const MIN_GRID_WIDTH: isize = 80;
const MIN_GRID_HEIGHT: isize = 24;

// TODO: Replace these with theme colors.
pub const COLOR_BLACK: Color = Color::from_hex(0x0C0C0CFF);
const COLOR_RED: Color = Color::from_hex(0xC50F1FFF);
const COLOR_GREEN: Color = Color::from_hex(0x13A10EFF);
const COLOR_YELLOW: Color = Color::from_hex(0xC19C00FF);
const COLOR_BLUE: Color = Color::from_hex(0x0037DAFF);
const COLOR_MAGENTA: Color = Color::from_hex(0x881798FF);
const COLOR_CYAN: Color = Color::from_hex(0x3A96DDFF);
const COLOR_WHITE: Color = Color::from_hex(0xCCCCCCFF);

const COLOR_BLACK_BRIGHT: Color = Color::from_hex(0x767676FF);
const COLOR_RED_BRIGHT: Color = Color::from_hex(0xE74856FF);
const COLOR_GREEN_BRIGHT: Color = Color::from_hex(0x16C60CFF);
const COLOR_YELLOW_BRIGHT: Color = Color::from_hex(0xF9F1A5FF);
const COLOR_BLUE_BRIGHT: Color = Color::from_hex(0x3B78FFFF);
const COLOR_MAGENTA_BRIGHT: Color = Color::from_hex(0xB4009EFF);
const COLOR_CYAN_BRIGHT: Color = Color::from_hex(0x61D6D6FF);
const COLOR_WHITE_BRIGHT: Color = Color::from_hex(0xF2F2F2FF);

pub struct TerminalEmulator {
    pty: Option<Pty>,

    // The position of the terminal's cursor, which follows different rules
    // compared to the document's cursor for compatibility reasons, and may be
    // different from the document's cursor position is the user is selecting text.
    grid_cursor: Position,
    grid_width: isize,
    grid_height: isize,
    grid_line_colors: Vec<Vec<(Color, Color)>>,

    doc_cursor_backups: Vec<(Position, Option<Selection>)>,

    is_cursor_visible: bool,
    foreground_color: Color,
    background_color: Color,
    are_colors_swapped: bool,
}

impl TerminalEmulator {
    pub fn new() -> Self {
        let grid_width = MIN_GRID_WIDTH;
        let grid_height = MIN_GRID_HEIGHT;

        let shells = [
            "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
            "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
            "C:\\Windows\\system32\\cmd.exe",
        ];

        let mut emulator = Self {
            pty: Pty::new(grid_width, grid_height, &shells).ok(),

            grid_cursor: Position::zero(),
            grid_width,
            grid_height,
            grid_line_colors: Vec::new(),

            doc_cursor_backups: Vec::new(),

            is_cursor_visible: false,
            foreground_color: COLOR_WHITE,
            background_color: COLOR_BLACK,
            are_colors_swapped: false,
        };

        emulator.resize_grid_line_colors();

        emulator
    }

    pub fn update_input(
        &mut self,
        widget: &Widget,
        ui: &mut UiHandle,
        doc: &mut Doc,
        tab: &mut Tab,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
    ) {
        let Some(mut pty) = self.pty.take() else {
            return;
        };

        let mut keybind_handler = widget.get_keybind_handler(ui);

        while let Some(keybind) = keybind_handler.next(ui.window) {
            match keybind {
                Keybind {
                    key: Key::Enter, ..
                } => {
                    pty.input.push('\r' as u32);
                }
                Keybind {
                    key: Key::Escape, ..
                } => {
                    pty.input.push(0x1B);
                }
                Keybind { key: Key::Tab, .. } => {
                    pty.input.push('\t' as u32);
                }
                Keybind {
                    key: Key::Backspace,
                    mods,
                } => {
                    let key_char = if mods & MOD_CTRL != 0 { 0x8 } else { 0x7F };

                    pty.input.extend_from_slice(&[key_char]);
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

                    pty.input.extend_from_slice(&[0x1B, LEFT_BRACKET]);

                    if mods != 0 {
                        pty.input.extend_from_slice(&[ONE, SEMICOLON]);
                    }

                    if mods & MOD_SHIFT != 0 && mods & MOD_CTRL != 0 {
                        pty.input.push(SIX);
                    } else if mods & MOD_SHIFT != 0 && mods & MOD_ALT != 0 {
                        pty.input.push(FOUR);
                    } else if mods & MOD_SHIFT != 0 {
                        pty.input.push(TWO);
                    } else if mods & MOD_CTRL != 0 {
                        pty.input.push(FIVE);
                    } else if mods & MOD_ALT != 0 {
                        pty.input.push(THREE);
                    }

                    pty.input.push(key_char as u32);
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
                        pty.input.push(keybind.key as u32 & 0x1F);
                    }
                }
                Keybind {
                    key: Key::V,
                    mods: MOD_CTRL,
                } => {
                    let text = ui.window.get_clipboard().unwrap_or(&[]);

                    for c in text {
                        pty.input.push(*c as u32);
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
                        pty.input.push(key & 0x1F);
                    }
                }
                _ => {}
            }
        }

        let mut char_handler = widget.get_char_handler(ui);

        while let Some(c) = char_handler.next(ui.window) {
            pty.input.push(c as u32);
        }

        pty.flush();

        self.pty = Some(pty);

        tab.update(widget, ui, doc, line_pool, text_buffer, config, time);
    }

    pub fn update_output(
        &mut self,
        ui: &mut UiHandle,
        doc: &mut Doc,
        tab: &mut Tab,
        line_pool: &mut LinePool,
        time: f32,
        dt: f32,
    ) {
        let Some(mut pty) = self.pty.take() else {
            return;
        };

        self.resize_grid(ui, tab, &mut pty);

        self.backup_doc_cursor_positions(doc);

        self.expand_doc_to_grid_size(doc, line_pool, time);

        if let Ok(mut output) = pty.output.try_lock() {
            self.handle_control_sequences(ui, doc, tab, &output, line_pool, time);

            output.clear();
        }

        self.restore_doc_cursor_positions(doc);

        self.pty = Some(pty);

        tab.update_camera(ui, doc, dt);
    }

    fn handle_control_sequences(
        &mut self,
        ui: &mut UiHandle,
        doc: &mut Doc,
        tab: &mut Tab,
        mut output: &[u32],
        line_pool: &mut LinePool,
        time: f32,
    ) {
        while !output.is_empty() {
            // Backspace:
            match output[0] {
                0x1B => {
                    if let Some(remaining) = output
                        .starts_with(&[0x1B, LEFT_BRACKET])
                        .then(|| {
                            self.handle_control_sequences_csi(doc, &output[2..], line_pool, time)
                        })
                        .flatten()
                    {
                        output = remaining;
                        continue;
                    }

                    if let Some(remaining) = output
                        .starts_with(&[0x1B, RIGHT_BRACKET])
                        .then(|| Self::handle_control_sequences_osc(&output[2..]))
                        .flatten()
                    {
                        output = remaining;
                        continue;
                    }
                }
                // Bell:
                0x7 => {
                    output = &output[1..];
                    continue;
                }
                // Backspace:
                0x8 => {
                    self.move_cursor(Position::new(-1, 0), doc);

                    output = &output[1..];
                    continue;
                }
                // Carriage Return:
                0xD => {
                    self.jump_cursor(Position::new(0, self.grid_cursor.y), doc);

                    output = &output[1..];
                    continue;
                }
                // Newline:
                0xA => {
                    if self.grid_cursor.y == self.grid_height - 1 {
                        self.scroll_grid(ui, tab, doc, line_pool, time);
                    } else {
                        self.move_cursor(Position::new(0, 1), doc);
                    }

                    output = &output[1..];
                    continue;
                }
                _ => {}
            }

            if let Some(c) = char::from_u32(output[0]) {
                self.insert_at_cursor(&[c], doc, line_pool, time);
            }

            output = &output[1..];
        }
    }

    fn handle_control_sequences_csi<'a>(
        &mut self,
        doc: &mut Doc,
        mut output: &'a [u32],
        line_pool: &mut LinePool,
        time: f32,
    ) -> Option<&'a [u32]> {
        match output.first() {
            Some(&QUESTION_MARK) => {
                output = &output[1..];

                let mut parameter_buffer = [0; 16];

                let parameters;
                (output, parameters) =
                    Self::parse_numeric_parameters(output, &mut parameter_buffer);

                match output.first() {
                    Some(&LOWERCASE_L) => {
                        if parameters.first() == Some(&25) {
                            self.is_cursor_visible = false;

                            Some(&output[1..])
                        } else {
                            None
                        }
                    }
                    Some(&LOWERCASE_H) => {
                        if parameters.first() == Some(&25) {
                            self.is_cursor_visible = true;
                            self.jump_doc_cursors_to_grid_cursor(doc);

                            Some(&output[1..])
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            _ => {
                let mut parameter_buffer = [0; 16];

                let parameters;
                (output, parameters) =
                    Self::parse_numeric_parameters(output, &mut parameter_buffer);

                match output.first() {
                    Some(&LOWERCASE_M) => {
                        // Set text formatting.
                        for parameter in parameters {
                            match *parameter {
                                0 => {
                                    self.foreground_color = COLOR_WHITE;
                                    self.background_color = COLOR_BLACK;
                                    self.are_colors_swapped = false;
                                }
                                7 => self.are_colors_swapped = true,
                                27 => self.are_colors_swapped = false,
                                30 => self.foreground_color = COLOR_BLACK,
                                31 => self.foreground_color = COLOR_RED,
                                32 => self.foreground_color = COLOR_GREEN,
                                33 => self.foreground_color = COLOR_YELLOW,
                                34 => self.foreground_color = COLOR_BLUE,
                                35 => self.foreground_color = COLOR_MAGENTA,
                                36 => self.foreground_color = COLOR_CYAN,
                                37 => self.foreground_color = COLOR_WHITE,
                                39 => self.foreground_color = COLOR_WHITE,
                                40 => self.background_color = COLOR_BLACK,
                                41 => self.background_color = COLOR_RED,
                                42 => self.background_color = COLOR_GREEN,
                                43 => self.background_color = COLOR_YELLOW,
                                44 => self.background_color = COLOR_BLUE,
                                45 => self.background_color = COLOR_MAGENTA,
                                46 => self.background_color = COLOR_CYAN,
                                47 => self.background_color = COLOR_WHITE,
                                49 => self.background_color = COLOR_BLACK,
                                90 => self.foreground_color = COLOR_BLACK_BRIGHT,
                                91 => self.foreground_color = COLOR_RED_BRIGHT,
                                92 => self.foreground_color = COLOR_GREEN_BRIGHT,
                                93 => self.foreground_color = COLOR_YELLOW_BRIGHT,
                                94 => self.foreground_color = COLOR_BLUE_BRIGHT,
                                95 => self.foreground_color = COLOR_MAGENTA_BRIGHT,
                                96 => self.foreground_color = COLOR_CYAN_BRIGHT,
                                97 => self.foreground_color = COLOR_WHITE_BRIGHT,
                                100 => self.background_color = COLOR_BLACK_BRIGHT,
                                101 => self.background_color = COLOR_RED_BRIGHT,
                                102 => self.background_color = COLOR_GREEN_BRIGHT,
                                103 => self.background_color = COLOR_YELLOW_BRIGHT,
                                104 => self.background_color = COLOR_BLUE_BRIGHT,
                                105 => self.background_color = COLOR_MAGENTA_BRIGHT,
                                106 => self.background_color = COLOR_CYAN_BRIGHT,
                                107 => self.background_color = COLOR_WHITE_BRIGHT,
                                _ => {}
                            }
                        }

                        Some(&output[1..])
                    }
                    Some(&UPPERCASE_H) => {
                        let y = parameters.first().copied().unwrap_or(1).saturating_sub(1);
                        let x = parameters.get(1).copied().unwrap_or(1).saturating_sub(1);

                        self.jump_cursor(Position::new(x as isize, y as isize), doc);

                        Some(&output[1..])
                    }
                    Some(&UPPERCASE_C) => {
                        let distance = parameters.first().copied().unwrap_or(0);

                        self.move_cursor(Position::new(distance as isize, 0), doc);

                        Some(&output[1..])
                    }
                    Some(&UPPERCASE_J) => {
                        if parameters.first() == Some(&2) {
                            // Clear screen.
                            let start = Position::zero();
                            let end = Position::new(self.grid_width, self.grid_height - 1);

                            self.delete(start, end, doc, line_pool, time);

                            Some(&output[1..])
                        } else {
                            None
                        }
                    }
                    Some(&UPPERCASE_K) => {
                        // Clear line after the cursor.
                        let start = self.grid_cursor;
                        let end = Position::new(self.grid_width, start.y);

                        self.delete(start, end, doc, line_pool, time);

                        Some(&output[1..])
                    }
                    Some(&UPPERCASE_X) => {
                        let distance = parameters.first().copied().unwrap_or(0);

                        // Clear characters after the cursor.
                        let start = self.grid_cursor;
                        let end = self.move_position(start, Position::new(distance as isize, 0));

                        self.delete(start, end, doc, line_pool, time);

                        Some(&output[1..])
                    }
                    _ => None,
                }
            }
        }
    }

    fn handle_control_sequences_osc(mut output: &[u32]) -> Option<&[u32]> {
        if output.starts_with(&[ZERO, SEMICOLON]) {
            // Setting the terminal title, ignored.
            output = &output[2..];

            loop {
                match output.first() {
                    Some(0x7) => break,
                    Some(0x1B) if output.get(1) == Some(&0x5C) => {
                        break;
                    }
                    None => break,
                    _ => {}
                }

                output = &output[1..];
            }

            Some(output)
        } else if output.starts_with(&[EIGHT, SEMICOLON]) {
            // Making text into a link, ignored.
            output = &output[3..];

            while !output.is_empty() && !output.starts_with(&[0x1B, BACK_SLASH]) {
                output = &output[1..];
            }

            if !output.is_empty() {
                output = &output[2..];
            }

            Some(output)
        } else {
            None
        }
    }

    fn parse_numeric_parameters<'a, 'b>(
        mut output: &'a [u32],
        parameter_buffer: &'b mut [usize; 16],
    ) -> (&'a [u32], &'b [usize]) {
        let mut parameter_count = 0;

        loop {
            let parameter;
            (output, parameter) = Self::parse_numeric_parameter(output);

            parameter_buffer[parameter_count] = parameter;
            parameter_count += 1;

            if output.first() == Some(&SEMICOLON) {
                output = &output[1..];
            } else {
                break;
            }
        }

        (output, &parameter_buffer[..parameter_count])
    }

    fn parse_numeric_parameter(mut output: &[u32]) -> (&[u32], usize) {
        let mut parameter = 0;

        while matches!(output[0], ZERO..=NINE) {
            parameter = parameter * 10 + (output[0] - ZERO) as usize;
            output = &output[1..];
        }

        (output, parameter)
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

            self.resize_grid_line_colors();
        }
    }

    fn resize_grid_line_colors(&mut self) {
        self.grid_line_colors.resize(
            self.grid_height as usize,
            Vec::with_capacity(self.grid_width as usize),
        );

        for y in 0..self.grid_height {
            self.grid_line_colors[y as usize]
                .resize(self.grid_width as usize, (COLOR_WHITE, COLOR_BLACK));
        }
    }

    fn scroll_grid(
        &mut self,
        ui: &mut UiHandle,
        tab: &mut Tab,
        doc: &mut Doc,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let start = doc.end();
        doc.insert(start, &['\n'], line_pool, time);

        for _ in 0..self.grid_width {
            let start = doc.end();
            doc.insert(start, &[' '], line_pool, time);
        }

        let first_grid_line = self.grid_line_colors.remove(0);
        self.grid_line_colors.push(first_grid_line);

        self.delete(
            Position::new(0, self.grid_height - 1),
            Position::new(self.grid_width, self.grid_height - 1),
            doc,
            line_pool,
            time,
        );

        let gfx = ui.gfx();

        let camera_offset_y =
            tab.camera.vertical.position - doc.lines().len() as f32 * gfx.line_height();

        let max_lines = self.grid_height as usize + MAX_SCROLLBACK_LINES;

        if doc.lines().len() > max_lines {
            let excess_lines = doc.lines().len() - max_lines;

            let start = Position::zero();
            let end = Position::new(0, excess_lines as isize);

            doc.delete(start, end, line_pool, time);
            doc.recycle_highlighted_lines_up_to_y(excess_lines);
        }

        tab.camera.vertical.position =
            doc.lines().len() as f32 * gfx.line_height() + camera_offset_y;

        tab.camera
            .vertical
            .recenter(CameraRecenterKind::OnScrollBorder);
    }

    fn expand_doc_to_grid_size(&mut self, doc: &mut Doc, line_pool: &mut LinePool, time: f32) {
        while (doc.lines().len() as isize) < self.grid_height {
            let start = doc.end();
            doc.insert(start, &['\n'], line_pool, time);
        }

        for y in 0..self.grid_height {
            let doc_y = doc.lines().len() as isize - self.grid_height + y;

            if doc.get_line_len(doc_y) >= self.grid_width {
                continue;
            }

            while doc.get_line_len(doc_y) < self.grid_width {
                let start = Position::new(doc.get_line_len(doc_y), doc_y);
                doc.insert(start, &[' '], line_pool, time);
            }

            doc.highlight_line_from_colors(&self.grid_line_colors[y as usize], doc_y as usize);
        }
    }

    fn clamp_position(&self, position: Position) -> Position {
        Position::new(
            position.x.clamp(0, self.grid_width - 1),
            position.y.clamp(0, self.grid_height - 1),
        )
    }

    fn move_position(&self, position: Position, delta: Position) -> Position {
        self.clamp_position(Position::new(position.x + delta.x, position.y + delta.y))
    }

    fn grid_position_to_doc_position(&self, position: Position, doc: &mut Doc) -> Position {
        Position::new(
            position.x,
            doc.lines().len() as isize - self.grid_height + position.y,
        )
    }

    fn doc_position_to_grid_position(&self, position: Position, doc: &mut Doc) -> Position {
        Position::new(
            position.x,
            position.y - (doc.lines().len() as isize - self.grid_height).max(0),
        )
    }

    fn backup_doc_cursor_positions(&mut self, doc: &mut Doc) {
        self.doc_cursor_backups.clear();

        for index in doc.cursor_indices() {
            let cursor = doc.get_cursor(index);
            let cursor_position = cursor.position;
            let cursor_selection = cursor.get_selection();

            let position = self.doc_position_to_grid_position(cursor_position, doc);

            let selection = cursor_selection.map(|selection| Selection {
                start: self.doc_position_to_grid_position(selection.start, doc),
                end: self.doc_position_to_grid_position(selection.end, doc),
            });

            self.doc_cursor_backups.push((position, selection));
        }
    }

    fn restore_doc_cursor_positions(&mut self, doc: &mut Doc) {
        for (index, (position, selection)) in doc.cursor_indices().zip(&self.doc_cursor_backups) {
            let Some(selection) = selection else {
                let doc_position = self.grid_position_to_doc_position(*position, doc);

                doc.jump_cursor(index, doc_position, false);

                continue;
            };

            let doc_selection_start = self.grid_position_to_doc_position(selection.start, doc);
            let doc_selection_end = self.grid_position_to_doc_position(selection.end, doc);

            if *position == selection.start {
                doc.jump_cursor(index, doc_selection_end, false);
                doc.jump_cursor(index, doc_selection_start, true);
            } else {
                doc.jump_cursor(index, doc_selection_start, false);
                doc.jump_cursor(index, doc_selection_end, true);
            }
        }
    }

    fn jump_doc_cursors_to_grid_cursor(&mut self, doc: &mut Doc) {
        if !self.is_cursor_visible {
            return;
        }

        self.doc_cursor_backups.clear();

        let doc_position =
            self.grid_position_to_doc_position(self.clamp_position(self.grid_cursor), doc);
        doc.jump_cursors(doc_position, false);
    }

    fn move_cursor(&mut self, delta: Position, doc: &mut Doc) {
        self.jump_cursor(
            Position::new(self.grid_cursor.x + delta.x, self.grid_cursor.y + delta.y),
            doc,
        );
    }

    fn jump_cursor(&mut self, position: Position, doc: &mut Doc) {
        self.grid_cursor = self.clamp_position(position);

        self.jump_doc_cursors_to_grid_cursor(doc);
    }

    fn insert_at_cursor(
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

    fn insert(
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

        for c in text {
            let next_position = self.move_position(position, Position::new(1, 0));

            {
                let position = self.grid_position_to_doc_position(position, doc);
                let next_position = self.grid_position_to_doc_position(next_position, doc);

                doc.delete(position, next_position, line_pool, time);
                doc.insert(position, &[*c], line_pool, time);
            }

            self.grid_line_colors[position.y as usize][position.x as usize] = colors;
            position = next_position;
        }

        self.jump_doc_cursors_to_grid_cursor(doc);

        let doc_start = self.grid_position_to_doc_position(start, doc);

        doc.highlight_line_from_colors(
            &self.grid_line_colors[start.y as usize],
            doc_start.y as usize,
        );
    }

    fn delete(
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

    pub fn on_close(&mut self) {
        self.pty.take();
    }

    pub fn pty(&self) -> Option<&Pty> {
        self.pty.as_ref()
    }
}
