use crate::{
    config::theme::Theme,
    geometry::position::Position,
    text::{
        doc::Doc,
        line_pool::LinePool,
        syntax_highlighter::{HighlightKind, TerminalHighlightKind},
    },
    ui::{color::Color, tab::Tab, terminal::color_table::COLOR_TABLE, UiHandle},
};

use super::{char32::*, terminal_emulator::TerminalEmulator};

impl TerminalEmulator {
    pub fn handle_escape_sequences(
        &mut self,
        ui: &mut UiHandle,
        docs: &mut (Doc, Doc),
        tab: &mut Tab,
        input: &mut Vec<u32>,
        mut output: &[u32],
        line_pool: &mut LinePool,
        theme: &Theme,
        time: f32,
    ) {
        while !output.is_empty() {
            let doc = self.get_doc_mut(docs);

            // Backspace:
            match output[0] {
                0x1B => {
                    let remaining = match output.get(1) {
                        Some(&LEFT_BRACKET) => self.handle_escape_sequences_csi(
                            doc,
                            input,
                            &output[2..],
                            line_pool,
                            time,
                        ),
                        Some(&RIGHT_BRACKET) => {
                            self.handle_escape_sequences_osc(input, &output[2..], theme)
                        }
                        Some(&EQUAL) => {
                            // Enter alternative keypad mode, ignored.
                            Some(&output[2..])
                        }
                        Some(&GREATER_THAN) => {
                            // Exit alternative keypad mode, ignored.
                            Some(&output[2..])
                        }
                        _ => None,
                    };

                    if let Some(remaining) = remaining {
                        output = remaining;
                        continue;
                    }

                    #[cfg(feature = "terminal_emulator_debug")]
                    {
                        // Print unhandled control sequences.
                        for c in output.iter().take(8) {
                            if let Some(c) = char::from_u32(*c) {
                                print!("{:?} ", c);
                            } else {
                                print!("{:?} ", c);
                            }
                        }

                        println!();
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
                    if self.grid_cursor.y == self.scroll_bottom {
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

    fn handle_escape_sequences_csi<'a>(
        &mut self,
        doc: &mut Doc,
        input: &mut Vec<u32>,
        mut output: &'a [u32],
        line_pool: &mut LinePool,
        time: f32,
    ) -> Option<&'a [u32]> {
        let mut parameter_buffer = [0; 16];

        match output.first() {
            Some(&QUESTION_MARK) => {
                output = &output[1..];

                let (output, parameters) =
                    Self::parse_numeric_parameters(output, &mut parameter_buffer);

                match output.first() {
                    Some(&LOWERCASE_L) => {
                        match parameters.first() {
                            Some(&25) => self.is_cursor_visible = false,
                            Some(&1047) | Some(&1049) => self.switch_to_normal_buffer(),
                            // Otherwise, ignored.
                            _ => {}
                        }

                        Some(&output[1..])
                    }
                    Some(&LOWERCASE_H) => {
                        match parameters.first() {
                            Some(&25) => {
                                self.is_cursor_visible = true;
                                self.jump_doc_cursors_to_grid_cursor(doc);
                            }
                            Some(&1047) | Some(&1049) => self.switch_to_alternate_buffer(),
                            // Otherwise, ignored.
                            _ => {}
                        }

                        Some(&output[1..])
                    }
                    Some(&LOWERCASE_M) => {
                        // Query xterm modifier key options.
                        let default_value = match parameters.first() {
                            Some(0) => 0, // modifyKeyboard
                            Some(1) => 2, // modifyCursorKeys
                            Some(2) => 2, // modifyFunctionKeys
                            Some(4) => 0, // modifyOtherKeys
                            _ => return None,
                        };

                        let response = format!("\u{1B}[>{}m", default_value);
                        input.extend(response.chars().map(|c| c as u32));

                        Some(&output[1..])
                    }
                    _ => None,
                }
            }
            Some(&GREATER_THAN) => {
                output = &output[1..];

                let (output, _) = Self::parse_numeric_parameters(output, &mut parameter_buffer);

                match output.first() {
                    Some(&LOWERCASE_M) => {
                        // Set/reset xterm modifier key options, ignored.
                        Some(&output[1..])
                    }
                    Some(&LOWERCASE_C) => {
                        // Query device attributes.
                        // According to invsible-island.net/xterm 41 corresponds to a VT420 which is the default.
                        let response = "\u{1B}[>41;0;0c";
                        input.extend(response.chars().map(|c| c as u32));

                        Some(&output[1..])
                    }
                    _ => None,
                }
            }
            _ => self.handle_unprefixed_escape_sequences_csi(doc, input, output, line_pool, time),
        }
    }

    fn handle_unprefixed_escape_sequences_csi<'a>(
        &mut self,
        doc: &mut Doc,
        input: &mut Vec<u32>,
        mut output: &'a [u32],
        line_pool: &mut LinePool,
        time: f32,
    ) -> Option<&'a [u32]> {
        let mut parameter_buffer = [0; 16];

        let parameters;
        (output, parameters) = Self::parse_numeric_parameters(output, &mut parameter_buffer);

        match output.first() {
            Some(&LOWERCASE_M) => {
                let parameters = if parameters.is_empty() {
                    &[0]
                } else {
                    parameters
                };

                let mut parameters = parameters.iter();

                // Set text formatting.
                while let Some(parameter) = parameters.next() {
                    match *parameter {
                        0 => {
                            self.foreground_color = TerminalHighlightKind::Foreground;
                            self.background_color = TerminalHighlightKind::Background;
                            self.are_colors_swapped = false;
                            self.are_colors_bright = false;
                        }
                        1 => self.are_colors_bright = true,
                        7 => self.are_colors_swapped = true,
                        22 => self.are_colors_bright = false,
                        27 => self.are_colors_swapped = false,
                        30 => self.foreground_color = TerminalHighlightKind::Background,
                        31 => self.foreground_color = TerminalHighlightKind::Red,
                        32 => self.foreground_color = TerminalHighlightKind::Green,
                        33 => self.foreground_color = TerminalHighlightKind::Yellow,
                        34 => self.foreground_color = TerminalHighlightKind::Blue,
                        35 => self.foreground_color = TerminalHighlightKind::Magenta,
                        36 => self.foreground_color = TerminalHighlightKind::Cyan,
                        37 => self.foreground_color = TerminalHighlightKind::Foreground,
                        38 => {
                            let color = Self::parse_color_from_parameters(&mut parameters)?;
                            self.foreground_color = TerminalHighlightKind::Custom(color);
                        }
                        39 => self.foreground_color = TerminalHighlightKind::Foreground,
                        40 => self.background_color = TerminalHighlightKind::Background,
                        41 => self.background_color = TerminalHighlightKind::Red,
                        42 => self.background_color = TerminalHighlightKind::Green,
                        43 => self.background_color = TerminalHighlightKind::Yellow,
                        44 => self.background_color = TerminalHighlightKind::Blue,
                        45 => self.background_color = TerminalHighlightKind::Magenta,
                        46 => self.background_color = TerminalHighlightKind::Cyan,
                        47 => self.background_color = TerminalHighlightKind::Foreground,
                        48 => {
                            let color = Self::parse_color_from_parameters(&mut parameters)?;
                            self.background_color = TerminalHighlightKind::Custom(color);
                        }
                        49 => self.background_color = TerminalHighlightKind::Background,
                        90 => self.foreground_color = TerminalHighlightKind::BrightBackground,
                        91 => self.foreground_color = TerminalHighlightKind::BrightRed,
                        92 => self.foreground_color = TerminalHighlightKind::BrightGreen,
                        93 => self.foreground_color = TerminalHighlightKind::BrightYellow,
                        94 => self.foreground_color = TerminalHighlightKind::BrightBlue,
                        95 => self.foreground_color = TerminalHighlightKind::BrightMagenta,
                        96 => self.foreground_color = TerminalHighlightKind::BrightCyan,
                        97 => self.foreground_color = TerminalHighlightKind::BrightForeground,
                        100 => self.background_color = TerminalHighlightKind::BrightBackground,
                        101 => self.background_color = TerminalHighlightKind::BrightRed,
                        102 => self.background_color = TerminalHighlightKind::BrightGreen,
                        103 => self.background_color = TerminalHighlightKind::BrightYellow,
                        104 => self.background_color = TerminalHighlightKind::BrightBlue,
                        105 => self.background_color = TerminalHighlightKind::BrightMagenta,
                        106 => self.background_color = TerminalHighlightKind::BrightCyan,
                        107 => self.background_color = TerminalHighlightKind::BrightForeground,
                        _ => {
                            #[cfg(feature = "terminal_emulator_debug")]
                            println!("Unhandled format parameter: {:?}", parameter);
                        }
                    }
                }

                Some(&output[1..])
            }
            Some(&UPPERCASE_H) => {
                let y = Self::get_parameter(parameters, 0, 1).saturating_sub(1);
                let x = Self::get_parameter(parameters, 1, 1).saturating_sub(1);

                self.jump_cursor(Position::new(x as isize, y as isize), doc);

                Some(&output[1..])
            }
            Some(&UPPERCASE_A) => {
                let distance = Self::get_parameter(parameters, 0, 1) as isize;
                self.move_cursor(Position::new(0, -distance), doc);

                Some(&output[1..])
            }
            Some(&UPPERCASE_B) => {
                let distance = Self::get_parameter(parameters, 0, 1) as isize;
                self.move_cursor(Position::new(0, distance), doc);

                Some(&output[1..])
            }
            Some(&UPPERCASE_C) => {
                let distance = Self::get_parameter(parameters, 0, 1) as isize;
                self.move_cursor(Position::new(distance, 0), doc);

                Some(&output[1..])
            }
            Some(&UPPERCASE_D) => {
                let distance = Self::get_parameter(parameters, 0, 1) as isize;
                self.move_cursor(Position::new(-distance, 0), doc);

                Some(&output[1..])
            }
            Some(&UPPERCASE_J) => {
                let (start, end) = match Self::get_parameter(parameters, 0, 0) {
                    0 => {
                        // Clear from the cursor to the end of the screen.
                        let start = self.grid_cursor;
                        let end = Position::new(self.grid_width, self.grid_height - 1);

                        Some((start, end))
                    }
                    1 => {
                        // Clear from the cursor to the beginning of the screen.
                        let start = Position::zero();
                        let end = self.grid_cursor;

                        Some((start, end))
                    }
                    2 => {
                        // Clear screen.
                        let start = Position::zero();
                        let end = Position::new(self.grid_width, self.grid_height - 1);

                        Some((start, end))
                    }
                    _ => None,
                }?;

                self.delete(start, end, doc, line_pool, time);

                Some(&output[1..])
            }
            Some(&UPPERCASE_K) => {
                let (start, end) = match Self::get_parameter(parameters, 0, 0) {
                    0 => {
                        // Clear from the cursor to the end of the line.
                        let start = self.grid_cursor;
                        let end = Position::new(self.grid_width, start.y);

                        Some((start, end))
                    }
                    1 => {
                        // Clear from the cursor to the beginning of the line.
                        let start = Position::new(0, self.grid_cursor.y);
                        let end = Position::new(self.grid_cursor.x, start.y);

                        Some((start, end))
                    }
                    2 => {
                        // Clear line.
                        let start = Position::new(0, self.grid_cursor.y);
                        let end = Position::new(self.grid_width, start.y);

                        Some((start, end))
                    }
                    _ => None,
                }?;

                self.delete(start, end, doc, line_pool, time);

                Some(&output[1..])
            }
            Some(&UPPERCASE_L) => {
                // Insert lines.
                let count = Self::get_parameter(parameters, 0, 1);

                let scroll_top = self.scroll_top.max(self.grid_cursor.y);
                let scroll_bottom = self.scroll_bottom;

                if scroll_top > scroll_bottom {
                    return None;
                }

                for _ in 0..count {
                    self.scroll_grid_region_down(scroll_top..=scroll_bottom, doc, line_pool, time);
                }

                Some(&output[1..])
            }
            Some(&UPPERCASE_M) => {
                // Delete lines.
                let count = Self::get_parameter(parameters, 0, 1);

                let scroll_top = self.scroll_top.max(self.grid_cursor.y);
                let scroll_bottom = self.scroll_bottom;

                if scroll_top > scroll_bottom {
                    return None;
                }

                for _ in 0..count {
                    self.scroll_grid_region_up(scroll_top..=scroll_bottom, doc, line_pool, time);
                }

                Some(&output[1..])
            }
            Some(&UPPERCASE_X) => {
                let distance = Self::get_parameter(parameters, 0, 1);

                // Clear characters after the cursor.
                let start = self.grid_cursor;
                let end = self.move_position(start, Position::new(distance as isize, 0));

                self.delete(start, end, doc, line_pool, time);

                Some(&output[1..])
            }
            Some(&SPACE) => {
                output = &output[1..];

                if output.first() == Some(&LOWERCASE_Q) {
                    // Set cursor shape, ignored.
                    Some(&output[1..])
                } else {
                    None
                }
            }
            Some(&LOWERCASE_T) => {
                // Xterm window controls, ignored.
                Some(&output[1..])
            }
            Some(&LOWERCASE_R) => {
                // Set scroll region.
                let top = Self::get_parameter(parameters, 0, 1) as isize - 1;
                let bottom =
                    Self::get_parameter(parameters, 1, self.grid_height as usize) as isize - 1;

                self.scroll_bottom = bottom.clamp(0, self.grid_height - 1);
                self.scroll_top = top.clamp(0, self.scroll_bottom);

                Some(&output[1..])
            }
            Some(&LOWERCASE_N) => {
                // Device status report.

                if parameters.first() == Some(&6) {
                    // Report cursor position (1-based).
                    let response = format!(
                        "\u{1B}[{};{}R",
                        self.grid_cursor.y + 1,
                        self.grid_cursor.x + 1
                    );

                    input.extend(response.chars().map(|c| c as u32));

                    Some(&output[1..])
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn handle_escape_sequences_osc<'a>(
        &self,
        input: &mut Vec<u32>,
        output: &'a [u32],
        theme: &Theme,
    ) -> Option<&'a [u32]> {
        let (mut output, kind) = Self::parse_numeric_parameter(output)?;

        if !output.starts_with(&[SEMICOLON]) {
            return None;
        }

        output = &output[1..];

        match kind {
            0 => {
                // Setting the terminal title, ignored.
                output = &output[2..];

                loop {
                    if let Some(remaining) = Self::consume_string_terminator(output) {
                        output = remaining;
                        break;
                    }

                    output = &output[1..];
                }

                Some(output)
            }
            8 => {
                // Making text into a link, ignored.
                output = &output[3..];

                loop {
                    if output.is_empty() {
                        break;
                    }

                    if let Some(remaining) = Self::consume_string_terminator(output) {
                        output = remaining;
                        break;
                    }

                    output = &output[1..];
                }

                Some(output)
            }
            10 | 11 => {
                // Setting/requesting foreground/background color.

                if !output.starts_with(&[QUESTION_MARK]) {
                    // Only requesting the value is supported.
                    return None;
                }

                output = &output[1..];
                output = Self::consume_string_terminator(output)?;

                let color = if kind == 10 {
                    self.foreground_color
                } else {
                    self.background_color
                };

                let color = theme.highlight_kind_to_color(HighlightKind::Terminal(color));

                let response = format!(
                    "\u{1B}]{};rgb:{:2X}{:2X}{:2X}\u{07}",
                    kind, color.r, color.g, color.b
                );

                input.extend(response.chars().map(|c| c as u32));

                Some(output)
            }
            _ => None,
        }
    }

    fn consume_string_terminator(output: &[u32]) -> Option<&[u32]> {
        if output.starts_with(&[0x7]) {
            Some(&output[1..])
        } else if output.starts_with(&[0x1B, BACK_SLASH]) {
            Some(&output[2..])
        } else {
            None
        }
    }

    fn parse_color_from_parameters<'a>(
        parameters: &mut impl Iterator<Item = &'a usize>,
    ) -> Option<Color> {
        let kind = parameters.next()?;

        match kind {
            2 => {
                // RGB true color:
                let r = (*parameters.next()?).clamp(0, 256);
                let g = (*parameters.next()?).clamp(0, 256);
                let b = (*parameters.next()?).clamp(0, 256);

                Some(Color::from_rgb(r as u8, g as u8, b as u8))
            }
            5 => {
                // 256 color table:
                let index = (*parameters.next()?).clamp(0, COLOR_TABLE.len());

                Some(Color::from_hex(COLOR_TABLE[index]))
            }
            _ => return None,
        }
    }

    fn parse_numeric_parameters<'a, 'b>(
        mut output: &'a [u32],
        parameter_buffer: &'b mut [usize; 16],
    ) -> (&'a [u32], &'b [usize]) {
        let mut parameter_count = 0;

        loop {
            let Some((next_output, parameter)) = Self::parse_numeric_parameter(output) else {
                break;
            };

            output = next_output;

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

    fn parse_numeric_parameter(mut output: &[u32]) -> Option<(&[u32], usize)> {
        let mut parameter = 0;

        if !matches!(output[0], ZERO..=NINE) {
            return None;
        }

        while matches!(output[0], ZERO..=NINE) {
            parameter = parameter * 10 + (output[0] - ZERO) as usize;
            output = &output[1..];
        }

        Some((output, parameter))
    }

    fn get_parameter(parameters: &[usize], index: usize, default: usize) -> usize {
        parameters.get(index).copied().unwrap_or(default)
    }
}
