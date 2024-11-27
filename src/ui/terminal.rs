use crate::{
    config::Config,
    geometry::{position::Position, rect::Rect},
    input::{key::Key, keybind::Keybind},
    platform::{gfx::Gfx, pty::Pty, window::Window},
    temp_buffer::TempBuffer,
    text::{
        cursor_index::CursorIndex,
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
};

use super::tab::Tab;

pub struct Terminal {
    tab: Tab,
    doc: Doc,
    pty: Option<Pty>,

    bounds: Rect,
}

impl Terminal {
    pub fn new(line_pool: &mut LinePool) -> Self {
        Self {
            tab: Tab::new(0),
            doc: Doc::new(line_pool, DocKind::Output),
            pty: Pty::new(240, 24).ok(),

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
        let Some(pty) = self.pty.as_mut() else {
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
                    pty.input.extend_from_slice(&['\r' as u32, '\n' as u32]);
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

        let mut mousebind_handler = window.get_mousebind_handler();

        while mousebind_handler.next(window).is_some() {}

        pty.flush();

        if let Ok(mut output) = pty.output.try_lock() {
            Self::handle_control_sequences(
                &output,
                &mut self.doc,
                pty.width(),
                pty.height(),
                line_pool,
                time,
            );

            output.clear();
        }

        self.tab
            .update(&mut self.doc, window, line_pool, text_buffer, config, time);

        self.tab.update_camera(&self.doc, window, dt);
    }

    pub fn draw(&mut self, config: &Config, gfx: &mut Gfx, is_focused: bool) {
        self.tab.draw(&mut self.doc, config, gfx, is_focused);
    }

    fn handle_control_sequences(
        mut output: &[u32],
        doc: &mut Doc,
        width: isize,
        height: isize,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        while !output.is_empty() {
            let cursor_position = doc.get_cursor(CursorIndex::Main).position;

            while (doc.lines().len() as isize) < height {
                let start = doc.end();
                doc.insert(start, &['\n'], line_pool, time);
            }

            for y in (doc.lines().len() as isize - height)..(doc.lines().len() as isize) {
                while doc.get_line_len(y) < width {
                    let start = Position::new(doc.get_line_len(y), y);
                    doc.insert(start, &[' '], line_pool, time);
                }
            }

            doc.jump_cursors(cursor_position, false);

            // Backspace:
            match output[0] {
                0x1B => {
                    if let Some(remaining) = output
                        .starts_with(&[0x1B, '[' as u32])
                        .then(|| {
                            Self::handle_control_sequences_csi(
                                &output[2..],
                                doc,
                                height,
                                line_pool,
                                time,
                            )
                        })
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
                // Backspace:
                0x8 => {
                    doc.move_cursors(Position::new(-1, 0), false);
                    output = &output[1..];
                    continue;
                }
                // Carriage Return:
                0xD => {
                    let cursor_position = doc.get_cursor(CursorIndex::Main).position;
                    doc.jump_cursors(Position::new(0, cursor_position.y), false);

                    output = &output[1..];
                    continue;
                }
                // Newline:
                0xA => {
                    let cursor_position = doc.get_cursor(CursorIndex::Main).position;

                    if cursor_position.y == doc.lines().len() as isize - 1 {
                        let start = doc.end();
                        doc.insert(start, &['\n'], line_pool, time);
                    }

                    doc.jump_cursors(
                        Position::new(cursor_position.x, cursor_position.y + 1),
                        false,
                    );

                    output = &output[1..];
                    continue;
                }
                _ => {}
            }

            if let Some(c) = char::from_u32(output[0]).filter(|c| !c.is_ascii_control()) {
                let start = doc.get_cursor(CursorIndex::Main).position;
                let end = doc.move_position(start, Position::new(1, 0));

                doc.delete(start, end, line_pool, time);
                doc.insert_at_cursors(&[c], line_pool, time);
            } else {
                println!("unknown char: {:#x}", output[0]);
            }

            output = &output[1..];
        }
    }

    fn handle_control_sequences_csi<'a>(
        mut output: &'a [u32],
        doc: &mut Doc,
        height: isize,
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
                            // Hide cursor, ignored.
                            println!("hiding cursor");
                            Some(&output[1..])
                        } else {
                            None
                        }
                    }
                    Some(&LOWERCASE_H) => {
                        if parameters.first() == Some(&25) {
                            // Show cursor, ignored.
                            println!("showing cursor");
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
                        // Set text formatting, ignored.
                        Some(&output[1..])
                    }
                    Some(&UPPERCASE_H) => {
                        let y = parameters.first().copied().unwrap_or(1).saturating_sub(1);
                        let x = parameters.get(1).copied().unwrap_or(1).saturating_sub(1);

                        let y = (doc.lines().len() as isize - height).max(0) + y as isize;

                        doc.jump_cursors(Position::new(x as isize, y), false);

                        Some(&output[1..])
                    }
                    Some(&UPPERCASE_C) => {
                        let distance = parameters.first().copied().unwrap_or(0);

                        doc.move_cursors(Position::new(distance as isize, 0), false);

                        Some(&output[1..])
                    }
                    Some(&UPPERCASE_J) => {
                        if parameters.first() == Some(&2) {
                            // Clear screen.
                            doc.clear(line_pool);

                            let cursor_position = doc.get_cursor(CursorIndex::Main).position;

                            let end = doc.end();
                            let start = Position::new(0, doc.end().y - height);

                            Self::clear_range(start, end, doc, line_pool, time);

                            doc.jump_cursors(cursor_position, false);

                            Some(&output[1..])
                        } else {
                            None
                        }
                    }
                    Some(&UPPERCASE_K) => {
                        // Clear line after the cursor.
                        let start = doc.get_cursor(CursorIndex::Main).position;
                        let end = Position::new(doc.get_line_len(start.y), start.y);

                        Self::clear_range(start, end, doc, line_pool, time);

                        doc.jump_cursors(start, false);

                        Some(&output[1..])
                    }
                    Some(&UPPERCASE_X) => {
                        let distance = parameters.first().copied().unwrap_or(0);

                        // Clear characters after the cursor.
                        let start = doc.get_cursor(CursorIndex::Main).position;
                        let end = doc.move_position(start, Position::new(distance as isize, 0));

                        Self::clear_range(start, end, doc, line_pool, time);

                        doc.jump_cursors(start, false);

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

    fn clear_range(
        start: Position,
        end: Position,
        doc: &mut Doc,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let mut position = start;

        while position < end {
            let next_position = doc.move_position(position, Position::new(1, 0));

            doc.delete(position, next_position, line_pool, time);
            doc.insert(position, &[' '], line_pool, time);

            position = next_position;
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
