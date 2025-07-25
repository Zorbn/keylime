use std::str;

use crate::{
    config::theme::Theme,
    ctx::Ctx,
    geometry::position::Position,
    pool::format_pooled,
    text::{
        doc::Doc,
        syntax_highlighter::{HighlightKind, TerminalHighlightKind},
    },
    ui::{color::Color, terminal::color_table::COLOR_TABLE},
};

use super::{terminal_emulator::TerminalEmulator, TerminalDocs, TERMINAL_DISPLAY_NAME};

impl TerminalEmulator {
    pub fn handle_escape_sequences(
        &mut self,
        docs: &mut TerminalDocs,
        input: &mut Vec<u8>,
        mut output: &[u8],
        ctx: &mut Ctx,
    ) {
        let mut plain_output = output;

        while !output.is_empty() {
            let doc = self.doc_mut(docs);
            let mut reset_plain_output = true;

            match output[0] {
                0x1B => {
                    self.flush_plain_output(plain_output, output, doc, ctx);

                    let remaining = match output.get(1) {
                        Some(&b'[') => {
                            self.handle_escape_sequences_csi(doc, input, &output[2..], ctx)
                        }
                        Some(&b']') => self.handle_escape_sequences_osc(
                            doc,
                            input,
                            &output[2..],
                            &ctx.config.theme,
                        ),
                        Some(&b'(') => {
                            match output.get(2) {
                                Some(&b'B') => {
                                    // Use ASCII character set (other character sets are unsupported).
                                    Some(&output[3..])
                                }
                                _ => None,
                            }
                        }
                        Some(&b'=') => {
                            // Enter alternative keypad mode, ignored.
                            Some(&output[2..])
                        }
                        Some(&b'>') => {
                            // Exit alternative keypad mode, ignored.
                            Some(&output[2..])
                        }
                        Some(&b'M') => {
                            self.reverse_newline_cursor(doc, ctx);

                            Some(&output[2..])
                        }
                        _ => None,
                    };

                    if let Some(remaining) = remaining {
                        output = remaining;
                    } else {
                        #[cfg(feature = "terminal_debug")]
                        {
                            // Print unhandled control sequences.
                            for c in output.iter().take(8) {
                                if let Some(c) = char::from_u32(*c as u32) {
                                    print!("{:?} ", c);
                                } else {
                                    print!("{:?} ", c);
                                }
                            }

                            println!();
                        }

                        output = &output[1..];
                    }
                }
                // Bell:
                0x7 => {
                    self.flush_plain_output(plain_output, output, doc, ctx);

                    output = &output[1..];
                }
                // Backspace:
                0x8 => {
                    self.flush_plain_output(plain_output, output, doc, ctx);

                    self.move_cursor(-1, 0, doc, ctx.gfx);

                    output = &output[1..];
                }
                // Tab:
                b'\t' => {
                    self.flush_plain_output(plain_output, output, doc, ctx);

                    let next_tab_stop = (self.grid_cursor.x / 8 + 1) * 8;

                    while self.grid_cursor.x < next_tab_stop {
                        self.insert_at_cursor(" ", doc, ctx);
                    }

                    output = &output[1..];
                }
                // Carriage Return:
                b'\r' => {
                    self.flush_plain_output(plain_output, output, doc, ctx);

                    self.jump_cursor(Position::new(0, self.grid_cursor.y), doc, ctx.gfx);

                    output = &output[1..];
                }
                // Newline:
                b'\n' => {
                    self.flush_plain_output(plain_output, output, doc, ctx);

                    self.newline_cursor(doc, ctx);

                    output = &output[1..];
                }
                _ => {
                    output = &output[1..];
                    reset_plain_output = false;
                }
            }

            if reset_plain_output {
                plain_output = output;
            }
        }

        let doc = self.doc_mut(docs);

        self.flush_plain_output(plain_output, output, doc, ctx);
        self.highlight_lines(doc);
    }

    fn flush_plain_output(
        &mut self,
        plain_output: &[u8],
        output: &[u8],
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) {
        let plain_len = output.as_ptr() as usize - plain_output.as_ptr() as usize;

        if plain_len == 0 {
            return;
        }

        let plain_string = Self::valid_utf8_range(&plain_output[..plain_len]);

        self.insert_at_cursor(plain_string, doc, ctx);
    }

    fn valid_utf8_range(bytes: &[u8]) -> &str {
        match str::from_utf8(bytes) {
            Ok(string) => string,
            Err(err) => unsafe { str::from_utf8_unchecked(&bytes[..err.valid_up_to()]) },
        }
    }

    fn handle_escape_sequences_csi<'a>(
        &mut self,
        doc: &mut Doc,
        input: &mut Vec<u8>,
        mut output: &'a [u8],
        ctx: &mut Ctx,
    ) -> Option<&'a [u8]> {
        let mut parameter_buffer = [0; 16];

        match output.first() {
            Some(&b'?') => {
                output = &output[1..];

                let (output, parameters) =
                    Self::parse_numeric_parameters(output, &mut parameter_buffer);

                match output.first() {
                    Some(&b'l') => {
                        match parameters.first() {
                            Some(&25) => self.is_cursor_visible = false,
                            Some(&1047) | Some(&1049) => self.switch_to_normal_buffer(doc),
                            // Otherwise, ignored.
                            #[cfg(feature = "terminal_debug")]
                            Some(parameter) => {
                                println!("Unhandled private mode disabled: {}", parameter)
                            }
                            _ => {}
                        }

                        Some(&output[1..])
                    }
                    Some(&b'h') => {
                        match parameters.first() {
                            Some(&25) => {
                                self.is_cursor_visible = true;
                                self.jump_doc_cursors_to_grid_cursor(doc, ctx.gfx);
                            }
                            Some(&1047) | Some(&1049) => self.switch_to_alternate_buffer(doc),
                            // Otherwise, ignored.
                            #[cfg(feature = "terminal_debug")]
                            Some(parameter) => {
                                println!("Unhandled private mode enabled: {}", parameter)
                            }
                            _ => {}
                        }

                        Some(&output[1..])
                    }
                    Some(&b'm') => {
                        // Query xterm modifier key options.
                        let default_value = match parameters.first() {
                            Some(0) => 0, // modifyKeyboard
                            Some(1) => 2, // modifyCursorKeys
                            Some(2) => 2, // modifyFunctionKeys
                            Some(4) => 0, // modifyOtherKeys
                            _ => return None,
                        };

                        let response = format_pooled!("\x1B[>{}m", default_value);

                        input.extend(response.bytes());

                        Some(&output[1..])
                    }
                    _ => None,
                }
            }
            Some(&b'>') => {
                output = &output[1..];

                let (output, _) = Self::parse_numeric_parameters(output, &mut parameter_buffer);

                match output.first() {
                    Some(&b'm') => {
                        // Set/reset xterm modifier key options, ignored.
                        Some(&output[1..])
                    }
                    Some(&b'c') => {
                        // Query device attributes.
                        // According to invisible-island.net/xterm 41 corresponds to a VT420 which is the default.
                        let response = "\x1B[>41;0;0c";
                        input.extend(response.bytes());

                        Some(&output[1..])
                    }
                    _ => None,
                }
            }
            _ => self.handle_unprefixed_escape_sequences_csi(doc, input, output, ctx),
        }
    }

    fn handle_unprefixed_escape_sequences_csi<'a>(
        &mut self,
        doc: &mut Doc,
        input: &mut Vec<u8>,
        mut output: &'a [u8],
        ctx: &mut Ctx,
    ) -> Option<&'a [u8]> {
        let mut parameter_buffer = [0; 16];

        let parameters;
        (output, parameters) = Self::parse_numeric_parameters(output, &mut parameter_buffer);

        match output.first() {
            Some(&b'm') => {
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
                            #[cfg(feature = "terminal_debug")]
                            println!("Unhandled format parameter: {:?}", parameter);
                        }
                    }
                }

                Some(&output[1..])
            }
            Some(&b'l') => {
                match parameters.first() {
                    // Otherwise, ignored.
                    #[cfg(feature = "terminal_debug")]
                    Some(parameter) => {
                        println!("Unhandled mode disabled: {}", parameter)
                    }
                    _ => {}
                }

                Some(&output[1..])
            }
            Some(&b'h') => {
                match parameters.first() {
                    // Otherwise, ignored.
                    #[cfg(feature = "terminal_debug")]
                    Some(parameter) => {
                        println!("Unhandled mode enabled: {}", parameter)
                    }
                    _ => {}
                }

                Some(&output[1..])
            }
            Some(&b'G') => {
                let x = Self::parameter(parameters, 0, 1).saturating_sub(1);
                let position =
                    self.grid_position_char_to_byte(Position::new(x, self.grid_cursor.y), doc);

                self.jump_cursor(position, doc, ctx.gfx);

                Some(&output[1..])
            }
            Some(&b'd') => {
                let y = Self::parameter(parameters, 0, 1).saturating_sub(1);
                let char_x = self.grid_position_byte_to_char(self.grid_cursor, doc);
                let position = self.grid_position_char_to_byte(Position::new(char_x, y), doc);

                self.jump_cursor(position, doc, ctx.gfx);

                Some(&output[1..])
            }
            Some(&b'H') => {
                let y = Self::parameter(parameters, 0, 1).saturating_sub(1);
                let x = Self::parameter(parameters, 1, 1).saturating_sub(1);
                let position = self.grid_position_char_to_byte(Position::new(x, y), doc);

                self.jump_cursor(position, doc, ctx.gfx);

                Some(&output[1..])
            }
            Some(&b'A') => {
                let distance = Self::parameter(parameters, 0, 1) as isize;
                self.move_cursor(0, -distance, doc, ctx.gfx);

                Some(&output[1..])
            }
            Some(&b'B') => {
                let distance = Self::parameter(parameters, 0, 1) as isize;
                self.move_cursor(0, distance, doc, ctx.gfx);

                Some(&output[1..])
            }
            Some(&b'C') => {
                let distance = Self::parameter(parameters, 0, 1) as isize;
                self.move_cursor(distance, 0, doc, ctx.gfx);

                Some(&output[1..])
            }
            Some(&b'D') => {
                let distance = Self::parameter(parameters, 0, 1) as isize;
                self.move_cursor(-distance, 0, doc, ctx.gfx);

                Some(&output[1..])
            }
            Some(&b'E') => {
                let distance = Self::parameter(parameters, 0, 1) as isize;
                let y = self.grid_cursor.y.saturating_add_signed(distance);
                self.jump_cursor(Position::new(0, y), doc, ctx.gfx);

                Some(&output[1..])
            }
            Some(&b'F') => {
                let distance = Self::parameter(parameters, 0, 1) as isize;
                let y = self.grid_cursor.y.saturating_add_signed(-distance);
                self.jump_cursor(Position::new(0, y), doc, ctx.gfx);

                Some(&output[1..])
            }
            Some(&b'J') => {
                let (start, end) = match Self::parameter(parameters, 0, 0) {
                    0 => {
                        // Clear from the cursor to the end of the screen.
                        let start = self.grid_cursor;
                        let end = self.line_end(self.grid_height - 1, doc);

                        Some((start, end))
                    }
                    1 => {
                        // Clear from the cursor to the beginning of the screen.
                        let start = Position::ZERO;
                        let end = self.grid_cursor;

                        Some((start, end))
                    }
                    2 => {
                        // Clear screen.
                        let start = Position::ZERO;
                        let end = self.line_end(self.grid_height - 1, doc);

                        Some((start, end))
                    }
                    _ => None,
                }?;

                self.delete(start, end, doc, ctx);

                Some(&output[1..])
            }
            Some(&b'K') => {
                let (start, end) = match Self::parameter(parameters, 0, 0) {
                    0 => {
                        // Clear from the cursor to the end of the line.
                        let start = self.grid_cursor;
                        let end = self.line_end(start.y, doc);

                        Some((start, end))
                    }
                    1 => {
                        // Clear from the cursor to the beginning of the line.
                        let start = Position::new(0, self.grid_cursor.y);
                        let end = Position::new(self.grid_cursor.x, self.grid_cursor.y);

                        Some((start, end))
                    }
                    2 => {
                        // Clear line.
                        let start = Position::new(0, self.grid_cursor.y);
                        let end = self.line_end(start.y, doc);

                        Some((start, end))
                    }
                    _ => None,
                }?;

                self.delete(start, end, doc, ctx);

                Some(&output[1..])
            }
            Some(&b'L') => {
                // Insert lines.
                let count = Self::parameter(parameters, 0, 1);

                let scroll_top = self.scroll_top.max(self.grid_cursor.y);
                let scroll_bottom = self.scroll_bottom;

                if scroll_top > scroll_bottom {
                    return None;
                }

                for _ in 0..count {
                    self.scroll_grid_region_down(scroll_top..=scroll_bottom, doc, ctx);
                }

                Some(&output[1..])
            }
            Some(&b'M') => {
                // Delete lines.
                let count = Self::parameter(parameters, 0, 1);

                let scroll_top = self.scroll_top.max(self.grid_cursor.y);
                let scroll_bottom = self.scroll_bottom;

                if scroll_top > scroll_bottom {
                    return None;
                }

                for _ in 0..count {
                    self.scroll_grid_region_up(scroll_top..=scroll_bottom, doc, ctx);
                }

                Some(&output[1..])
            }
            Some(&b'S') => {
                // Scroll up.
                let count = Self::parameter(parameters, 0, 1);

                for _ in 0..count {
                    self.scroll_grid_region_up(self.scroll_top..=self.scroll_bottom, doc, ctx);
                }

                Some(&output[1..])
            }
            Some(&b'T') => {
                // Scroll down.
                let count = Self::parameter(parameters, 0, 1);

                for _ in 0..count {
                    self.scroll_grid_region_down(self.scroll_top..=self.scroll_bottom, doc, ctx);
                }

                Some(&output[1..])
            }
            Some(&b'X') => {
                let distance = Self::parameter(parameters, 0, 1);

                // Clear characters after the cursor.
                let start = self.grid_cursor;
                let end = self.move_position(start, distance as isize, 0, doc);

                self.delete(start, end, doc, ctx);

                Some(&output[1..])
            }
            Some(&b'P') => {
                let distance = Self::parameter(parameters, 0, 1);

                // Delete characters after the cursor, shifting the rest of the line over.
                let start = self.grid_cursor;
                let end = self.move_position(start, 1, 0, doc);

                let start = self.grid_position_to_doc_position(start, doc);
                let end = self.grid_position_to_doc_position(end, doc);

                if start.y != end.y {
                    return Some(&output[1..]);
                }

                for _ in 0..distance {
                    doc.delete(start, end, ctx);
                    doc.insert(doc.line_end(start.y), " ", ctx);
                }

                Some(&output[1..])
            }
            Some(&b' ') => {
                output = &output[1..];

                // Set cursor shape, ignored.
                (output.first() == Some(&b'q')).then_some(&output[1..])
            }
            Some(&b't') => {
                // Xterm window controls, ignored.
                Some(&output[1..])
            }
            Some(&b'r') => {
                // Set scroll region.
                let top = Self::parameter(parameters, 0, 1).saturating_sub(1);
                let bottom = Self::parameter(parameters, 1, self.grid_height).saturating_sub(1);

                self.scroll_bottom = bottom.clamp(0, self.grid_height - 1);
                self.scroll_top = top.clamp(0, self.scroll_bottom);

                Some(&output[1..])
            }
            Some(&b'n') => {
                // Device status report.

                (parameters.first() == Some(&6)).then(|| {
                    let char_x = self.grid_position_byte_to_char(self.grid_cursor, doc);

                    let response =
                        format_pooled!("\x1B[{};{}R", self.grid_cursor.y + 1, char_x + 1);

                    input.extend(response.bytes());

                    &output[1..]
                })
            }
            Some(&b'c') => {
                // Terminal ID.

                // Claim to be a "VT101 with no options".
                input.extend("\x1B[?1;0c".bytes());

                Some(&output[1..])
            }
            _ => None,
        }
    }

    fn handle_escape_sequences_osc<'a>(
        &self,
        doc: &mut Doc,
        input: &mut Vec<u8>,
        output: &'a [u8],
        theme: &Theme,
    ) -> Option<&'a [u8]> {
        let (mut output, kind) = Self::parse_numeric_parameter(output)?;

        if !output.starts_with(b";") {
            return None;
        }

        output = &output[1..];

        match kind {
            0 | 2 => {
                let title = Self::consume_terminated_string(&mut output);

                if let Some(title) = title
                    .and_then(|title| str::from_utf8(title).ok())
                    .filter(|title| !title.is_empty())
                {
                    doc.set_display_name(Some(title.into()));
                } else {
                    doc.set_display_name(Some(TERMINAL_DISPLAY_NAME.into()));
                }

                Some(output)
            }
            10 | 11 => {
                // Setting/requesting foreground/background color.

                if !output.starts_with(b"?") {
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

                let response = format_pooled!(
                    "\x1B]{};rgb:{:2X}{:2X}{:2X}\x07",
                    kind,
                    color.r,
                    color.g,
                    color.b
                );

                input.extend(response.bytes());

                Some(output)
            }
            _ => {
                #[cfg(feature = "terminal_debug")]
                println!("Unhandled osc sequence: {}", kind);

                Self::consume_terminated_string(&mut output);

                Some(output)
            }
        }
    }

    fn consume_terminated_string<'a>(output: &mut &'a [u8]) -> Option<&'a [u8]> {
        let string_bytes = *output;
        let mut string_len = 0;

        loop {
            if output.is_empty() {
                return None;
            }

            if let Some(remaining) = Self::consume_string_terminator(output) {
                *output = remaining;
                break;
            }

            *output = &output[1..];
            string_len += 1;
        }

        Some(&string_bytes[..string_len])
    }

    fn consume_string_terminator(output: &[u8]) -> Option<&[u8]> {
        if output.starts_with(&[0x7]) {
            Some(&output[1..])
        } else if output.starts_with(&[0x1B, b'\\']) {
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
            _ => None,
        }
    }

    fn parse_numeric_parameters<'a, 'b>(
        mut output: &'a [u8],
        parameter_buffer: &'b mut [usize; 16],
    ) -> (&'a [u8], &'b [usize]) {
        let mut parameter_count = 0;

        loop {
            let Some((next_output, parameter)) = Self::parse_numeric_parameter(output) else {
                break;
            };

            output = next_output;

            parameter_buffer[parameter_count] = parameter;
            parameter_count += 1;

            if output.first() == Some(&b';') {
                output = &output[1..];
            } else {
                break;
            }
        }

        (output, &parameter_buffer[..parameter_count])
    }

    fn parse_numeric_parameter(mut output: &[u8]) -> Option<(&[u8], usize)> {
        let mut parameter = 0;

        if !output.first()?.is_ascii_digit() {
            return None;
        }

        while output.first()?.is_ascii_digit() {
            parameter = parameter * 10 + (output[0] - b'0') as usize;
            output = &output[1..];
        }

        Some((output, parameter))
    }

    fn parameter(parameters: &[usize], index: usize, default: usize) -> usize {
        parameters.get(index).copied().unwrap_or(default)
    }
}
