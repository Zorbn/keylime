use crate::{
    config::Config,
    geometry::{position::Position, rect::Rect},
    input::{key::Key, keybind::Keybind},
    platform::{gfx::Gfx, pty::Pty, window::Window},
    temp_buffer::TempBuffer,
    text::{
        doc::{Doc, DocKind},
        line_pool::LinePool,
        selection::Selection,
    },
};

use super::{camera::CameraRecenterKind, color::Color, tab::Tab};

const MAX_SCROLLBACK_LINES: usize = 100;

// TODO: Replace these with theme colors.
const COLOR_BLACK: Color = Color::from_hex(0x000000FF);
const COLOR_RED: Color = Color::from_hex(0xFF0000FF);
const COLOR_GREEN: Color = Color::from_hex(0x00FF00FF);
const COLOR_YELLOW: Color = Color::from_hex(0xFFFF00FF);
const COLOR_BLUE: Color = Color::from_hex(0x0000FFFF);
const COLOR_MAGENTA: Color = Color::from_hex(0xFF00FFFF);
const COLOR_CYAN: Color = Color::from_hex(0x007DFFFF);
const COLOR_WHITE: Color = Color::from_hex(0xFFFFFFFF);

pub struct Terminal {
    tab: Tab,
    doc: Doc,
    pty: Option<Pty>,

    // The position of the terminal's cursor, which follows different rules
    // compared to the document's cursor for compatibility reasons, and may be
    // different from the document's cursor position is the user is selecting text.
    grid_cursor: Position,
    grid_width: isize,
    grid_height: isize,
    grid_line_colors: Vec<Vec<Color>>,

    doc_cursor_backups: Vec<(Position, Option<Selection>)>,

    is_cursor_visible: bool,
    foreground_color: Color,

    bounds: Rect,
}

impl Terminal {
    pub fn new(line_pool: &mut LinePool) -> Self {
        let grid_width = 240isize;
        let grid_height = 24isize;

        let mut grid_line_colors = Vec::with_capacity(grid_height as usize);

        for y in 0..grid_height {
            grid_line_colors.push(Vec::with_capacity(grid_width as usize));

            for _ in 0..grid_width {
                grid_line_colors[y as usize].push(COLOR_WHITE);
            }
        }

        Self {
            tab: Tab::new(0),
            doc: Doc::new(line_pool, DocKind::Output),
            pty: Pty::new(grid_width, grid_height).ok(),

            grid_cursor: Position::zero(),
            grid_width,
            grid_height,
            grid_line_colors,

            doc_cursor_backups: Vec::new(),

            is_cursor_visible: false,
            foreground_color: COLOR_WHITE,

            bounds: Rect::zero(),
        }
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx) {
        let height = (gfx.line_height() * 15.0).floor();

        self.bounds = Rect::new(bounds.x, bounds.bottom() - height, bounds.width, height);

        self.tab.layout(Rect::zero(), self.bounds, &self.doc, gfx);
    }

    pub fn update(
        &mut self,
        window: &mut Window,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
        dt: f32,
    ) {
        let Some(mut pty) = self.pty.take() else {
            return;
        };

        let mut char_handler = window.get_char_handler();

        while let Some(c) = char_handler.next(window) {
            pty.input.push(c as u32);
        }

        let mut keybind_handler = window.get_keybind_handler();

        while let Some(keybind) = keybind_handler.next(window) {
            match keybind {
                Keybind {
                    key: Key::Enter, ..
                } => {
                    pty.input.push('\r' as u32);
                }
                Keybind {
                    key: Key::Backspace,
                    ..
                } => {
                    pty.input.extend_from_slice(&[0x7F]);
                }
                _ => {}
            }
        }

        pty.flush();

        self.expand_doc_to_grid_size(line_pool, time);

        self.tab
            .update(&mut self.doc, window, line_pool, text_buffer, config, time);

        self.backup_doc_cursor_positions();

        if let Ok(mut output) = pty.output.try_lock() {
            self.handle_control_sequences(window, &output, line_pool, time);

            output.clear();
        }

        self.restore_doc_cursor_positions();

        self.pty = Some(pty);

        self.tab.update_camera(&self.doc, window, dt);
    }

    pub fn draw(&mut self, config: &Config, gfx: &mut Gfx, is_focused: bool) {
        self.tab.draw(&mut self.doc, config, gfx, is_focused);
    }

    fn handle_control_sequences(
        &mut self,
        window: &mut Window,
        mut output: &[u32],
        line_pool: &mut LinePool,
        time: f32,
    ) {
        while !output.is_empty() {
            // Backspace:
            match output[0] {
                0x1B => {
                    if let Some(remaining) = output
                        .starts_with(&[0x1B, '[' as u32])
                        .then(|| self.handle_control_sequences_csi(&output[2..], line_pool, time))
                        .flatten()
                    {
                        output = remaining;
                        continue;
                    }

                    if let Some(remaining) = output
                        .starts_with(&[0x1B, ']' as u32])
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
                    self.move_cursor(Position::new(-1, 0));

                    output = &output[1..];
                    continue;
                }
                // Carriage Return:
                0xD => {
                    self.jump_cursor(Position::new(0, self.grid_cursor.y));

                    output = &output[1..];
                    continue;
                }
                // Newline:
                0xA => {
                    if self.grid_cursor.y == self.grid_height - 1 {
                        let start = self.doc.end();
                        self.doc.insert(start, &['\n'], line_pool, time);

                        for _ in 0..self.grid_width {
                            let start = self.doc.end();
                            self.doc.insert(start, &[' '], line_pool, time);
                        }

                        let gfx = window.gfx();

                        let camera_offset_y = self.tab.camera.vertical.position
                            - self.doc.lines().len() as f32 * gfx.line_height();

                        let max_lines = self.grid_height as usize + MAX_SCROLLBACK_LINES;

                        if self.doc.lines().len() > max_lines {
                            let excess_lines = self.doc.lines().len() - max_lines;

                            let start = Position::zero();
                            let end = Position::new(0, excess_lines as isize);

                            self.doc.delete(start, end, line_pool, time);
                        }

                        self.tab.camera.vertical.position =
                            self.doc.lines().len() as f32 * gfx.line_height() + camera_offset_y;

                        self.tab
                            .camera
                            .vertical
                            .recenter(CameraRecenterKind::OnScrollBorder);
                    } else {
                        self.move_cursor(Position::new(0, 1));
                    }

                    output = &output[1..];
                    continue;
                }
                _ => {}
            }

            if let Some(c) = char::from_u32(output[0]) {
                self.insert_at_cursor(&[c], line_pool, time);
            }

            output = &output[1..];
        }
    }

    fn handle_control_sequences_csi<'a>(
        &mut self,
        mut output: &'a [u32],
        line_pool: &mut LinePool,
        time: f32,
    ) -> Option<&'a [u32]> {
        const QUESTION_MARK: u32 = '?' as u32;
        const LOWERCASE_L: u32 = 'l' as u32;
        const LOWERCASE_H: u32 = 'h' as u32;
        const LOWERCASE_M: u32 = 'm' as u32;
        const UPPERCASE_C: u32 = 'C' as u32;
        const UPPERCASE_H: u32 = 'H' as u32;
        const UPPERCASE_J: u32 = 'J' as u32;
        const UPPERCASE_K: u32 = 'K' as u32;
        const UPPERCASE_X: u32 = 'X' as u32;

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
                            self.jump_doc_cursors_to_grid_cursor();

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
                                }
                                30 | 90 => {
                                    self.foreground_color = COLOR_BLACK;
                                }
                                31 | 91 => {
                                    self.foreground_color = COLOR_RED;
                                }
                                32 | 92 => {
                                    self.foreground_color = COLOR_GREEN;
                                }
                                33 | 93 => {
                                    self.foreground_color = COLOR_YELLOW;
                                }
                                34 | 94 => {
                                    self.foreground_color = COLOR_BLUE;
                                }
                                35 | 95 => {
                                    self.foreground_color = COLOR_MAGENTA;
                                }
                                36 | 96 => {
                                    self.foreground_color = COLOR_CYAN;
                                }
                                37 | 97 => {
                                    self.foreground_color = COLOR_WHITE;
                                }
                                _ => {}
                            }
                        }

                        Some(&output[1..])
                    }
                    Some(&UPPERCASE_H) => {
                        let y = parameters.first().copied().unwrap_or(1).saturating_sub(1);
                        let x = parameters.get(1).copied().unwrap_or(1).saturating_sub(1);

                        self.jump_cursor(Position::new(x as isize, y as isize));

                        Some(&output[1..])
                    }
                    Some(&UPPERCASE_C) => {
                        let distance = parameters.first().copied().unwrap_or(0);

                        self.move_cursor(Position::new(distance as isize, 0));

                        Some(&output[1..])
                    }
                    Some(&UPPERCASE_J) => {
                        if parameters.first() == Some(&2) {
                            // Clear screen.
                            let start = Position::zero();
                            let end = Position::new(self.grid_width, self.grid_height - 1);

                            self.delete(start, end, line_pool, time);

                            Some(&output[1..])
                        } else {
                            None
                        }
                    }
                    Some(&UPPERCASE_K) => {
                        // Clear line after the cursor.
                        let start = self.grid_cursor;
                        let end = Position::new(self.grid_width, start.y);

                        self.delete(start, end, line_pool, time);

                        Some(&output[1..])
                    }
                    Some(&UPPERCASE_X) => {
                        let distance = parameters.first().copied().unwrap_or(0);

                        // Clear characters after the cursor.
                        let start = self.grid_cursor;
                        let end = self.move_position(start, Position::new(distance as isize, 0));

                        self.delete(start, end, line_pool, time);

                        Some(&output[1..])
                    }
                    _ => None,
                }
            }
        }
    }

    fn handle_control_sequences_osc(mut output: &[u32]) -> Option<&[u32]> {
        const ZERO: u32 = '0' as u32;
        const SEMICOLON: u32 = ';' as u32;

        if output.starts_with(&[ZERO, SEMICOLON]) {
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
        } else {
            None
        }
    }

    fn parse_numeric_parameters<'a, 'b>(
        mut output: &'a [u32],
        parameter_buffer: &'b mut [usize; 16],
    ) -> (&'a [u32], &'b [usize]) {
        const SEMICOLON: u32 = ';' as u32;

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
        const ZERO: u32 = '0' as u32;
        const NINE: u32 = '9' as u32;

        let mut parameter = 0;

        while matches!(output[0], ZERO..=NINE) {
            parameter = parameter * 10 + (output[0] - ZERO) as usize;
            output = &output[1..];
        }

        (output, parameter)
    }

    fn expand_doc_to_grid_size(&mut self, line_pool: &mut LinePool, time: f32) {
        while (self.doc.lines().len() as isize) < self.grid_height {
            let start = self.doc.end();
            self.doc.insert(start, &['\n'], line_pool, time);
        }

        for y in 0..self.grid_height {
            let y = self.doc.lines().len() as isize - self.grid_height + y;

            while self.doc.get_line_len(y) < self.grid_width {
                let start = Position::new(self.doc.get_line_len(y), y);
                self.doc.insert(start, &[' '], line_pool, time);
            }
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

    fn grid_position_to_doc_position(&self, position: Position) -> Position {
        Position::new(
            position.x,
            self.doc.lines().len() as isize - self.grid_height + position.y,
        )
    }

    fn doc_position_to_grid_position(&self, position: Position) -> Position {
        Position::new(
            position.x,
            position.y - (self.doc.lines().len() as isize - self.grid_height),
        )
    }

    fn backup_doc_cursor_positions(&mut self) {
        self.doc_cursor_backups.clear();

        for index in self.doc.cursor_indices() {
            let cursor = self.doc.get_cursor(index);

            let position = self.doc_position_to_grid_position(cursor.position);

            let selection = cursor.get_selection().map(|selection| Selection {
                start: self.doc_position_to_grid_position(selection.start),
                end: self.doc_position_to_grid_position(selection.end),
            });

            self.doc_cursor_backups.push((position, selection));
        }
    }

    fn restore_doc_cursor_positions(&mut self) {
        for (index, (position, selection)) in
            self.doc.cursor_indices().zip(&self.doc_cursor_backups)
        {
            let Some(selection) = selection else {
                self.doc
                    .jump_cursor(index, self.grid_position_to_doc_position(*position), false);

                continue;
            };

            if *position == selection.start {
                self.doc.jump_cursor(
                    index,
                    self.grid_position_to_doc_position(selection.end),
                    false,
                );
                self.doc.jump_cursor(
                    index,
                    self.grid_position_to_doc_position(selection.start),
                    true,
                );
            } else {
                self.doc.jump_cursor(
                    index,
                    self.grid_position_to_doc_position(selection.start),
                    false,
                );
                self.doc.jump_cursor(
                    index,
                    self.grid_position_to_doc_position(selection.end),
                    true,
                );
            }
        }
    }

    fn jump_doc_cursors_to_grid_cursor(&mut self) {
        if !self.is_cursor_visible {
            return;
        }

        self.doc_cursor_backups.clear();

        let doc_position = self.grid_position_to_doc_position(self.grid_cursor);
        self.doc.jump_cursors(doc_position, false);
    }

    fn move_cursor(&mut self, delta: Position) {
        self.jump_cursor(Position::new(
            self.grid_cursor.x + delta.x,
            self.grid_cursor.y + delta.y,
        ));
    }

    fn jump_cursor(&mut self, position: Position) {
        self.grid_cursor = self.clamp_position(position);

        self.jump_doc_cursors_to_grid_cursor();
    }

    fn insert_at_cursor(&mut self, text: &[char], line_pool: &mut LinePool, time: f32) {
        self.insert(self.grid_cursor, text, line_pool, time);
        self.move_cursor(Position::new(text.len() as isize, 0));
    }

    fn insert(&mut self, start: Position, text: &[char], line_pool: &mut LinePool, time: f32) {
        let mut position = start;

        for c in text {
            let next_position = self.move_position(position, Position::new(1, 0));

            {
                let position = self.grid_position_to_doc_position(position);
                let next_position = self.grid_position_to_doc_position(next_position);

                self.doc.delete(position, next_position, line_pool, time);
                self.doc.insert(position, &[*c], line_pool, time);
            }

            self.grid_line_colors[position.y as usize][position.x as usize] = self.foreground_color;
            position = next_position;
        }

        self.jump_doc_cursors_to_grid_cursor();

        let doc_start = self.grid_position_to_doc_position(start);

        self.doc.highlight_line_from_colors(
            &self.grid_line_colors[start.y as usize],
            doc_start.y as usize,
        );
    }

    fn delete(&mut self, start: Position, end: Position, line_pool: &mut LinePool, time: f32) {
        for y in start.y..=end.y {
            let start_x = if y == start.y { start.x } else { 0 };
            let end_x = if y == end.y { end.x } else { self.grid_width };

            for x in start_x..end_x {
                self.insert(Position::new(x, y), &[' '], line_pool, time);
            }
        }
    }

    pub fn on_close(&mut self) {
        self.pty.take();
    }

    pub fn pty(&self) -> Option<&Pty> {
        self.pty.as_ref()
    }

    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn is_animating(&self) -> bool {
        self.tab.is_animating()
    }
}
