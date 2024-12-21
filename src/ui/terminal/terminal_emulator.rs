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
    ui::{camera::CameraRecenterKind, color::Color, tab::Tab, widget::Widget, UiHandle},
};

const COLOR_TABLE: [u32; 256] = [
    0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xC0C0C0, 0x808080,
    0xFF0000, 0x00FF00, 0xFFFF00, 0x0000FF, 0xFF00FF, 0x00FFFF, 0xFFFFFF, 0x000000, 0x00005F,
    0x000087, 0x0000AF, 0x0000D7, 0x0000FF, 0x005F00, 0x005F5F, 0x005F87, 0x005FAF, 0x005FD7,
    0x005FFF, 0x008700, 0x00875F, 0x008787, 0x0087AF, 0x0087D7, 0x0087FF, 0x00AF00, 0x00AF5F,
    0x00AF87, 0x00AFAF, 0x00AFD7, 0x00AFFF, 0x00D700, 0x00D75F, 0x00D787, 0x00D7AF, 0x00D7D7,
    0x00D7FF, 0x00FF00, 0x00FF5F, 0x00FF87, 0x00FFAF, 0x00FFD7, 0x00FFFF, 0x5F0000, 0x5F005F,
    0x5F0087, 0x5F00AF, 0x5F00D7, 0x5F00FF, 0x5F5F00, 0x5F5F5F, 0x5F5F87, 0x5F5FAF, 0x5F5FD7,
    0x5F5FFF, 0x5F8700, 0x5F875F, 0x5F8787, 0x5F87AF, 0x5F87D7, 0x5F87FF, 0x5FAF00, 0x5FAF5F,
    0x5FAF87, 0x5FAFAF, 0x5FAFD7, 0x5FAFFF, 0x5FD700, 0x5FD75F, 0x5FD787, 0x5FD7AF, 0x5FD7D7,
    0x5FD7FF, 0x5FFF00, 0x5FFF5F, 0x5FFF87, 0x5FFFAF, 0x5FFFD7, 0x5FFFFF, 0x870000, 0x87005F,
    0x870087, 0x8700AF, 0x8700D7, 0x8700FF, 0x875F00, 0x875F5F, 0x875F87, 0x875FAF, 0x875FD7,
    0x875FFF, 0x878700, 0x87875F, 0x878787, 0x8787AF, 0x8787D7, 0x8787FF, 0x87AF00, 0x87AF5F,
    0x87AF87, 0x87AFAF, 0x87AFD7, 0x87AFFF, 0x87D700, 0x87D75F, 0x87D787, 0x87D7AF, 0x87D7D7,
    0x87D7FF, 0x87FF00, 0x87FF5F, 0x87FF87, 0x87FFAF, 0x87FFD7, 0x87FFFF, 0xAF0000, 0xAF005F,
    0xAF0087, 0xAF00AF, 0xAF00D7, 0xAF00FF, 0xAF5F00, 0xAF5F5F, 0xAF5F87, 0xAF5FAF, 0xAF5FD7,
    0xAF5FFF, 0xAF8700, 0xAF875F, 0xAF8787, 0xAF87AF, 0xAF87D7, 0xAF87FF, 0xAFAF00, 0xAFAF5F,
    0xAFAF87, 0xAFAFAF, 0xAFAFD7, 0xAFAFFF, 0xAFD700, 0xAFD75F, 0xAFD787, 0xAFD7AF, 0xAFD7D7,
    0xAFD7FF, 0xAFFF00, 0xAFFF5F, 0xAFFF87, 0xAFFFAF, 0xAFFFD7, 0xAFFFFF, 0xD70000, 0xD7005F,
    0xD70087, 0xD700AF, 0xD700D7, 0xD700FF, 0xD75F00, 0xD75F5F, 0xD75F87, 0xD75FAF, 0xD75FD7,
    0xD75FFF, 0xD78700, 0xD7875F, 0xD78787, 0xD787AF, 0xD787D7, 0xD787FF, 0xD7AF00, 0xD7AF5F,
    0xD7AF87, 0xD7AFAF, 0xD7AFD7, 0xD7AFFF, 0xD7D700, 0xD7D75F, 0xD7D787, 0xD7D7AF, 0xD7D7D7,
    0xD7D7FF, 0xD7FF00, 0xD7FF5F, 0xD7FF87, 0xD7FFAF, 0xD7FFD7, 0xD7FFFF, 0xFF0000, 0xFF005F,
    0xFF0087, 0xFF00AF, 0xFF00D7, 0xFF00FF, 0xFF5F00, 0xFF5F5F, 0xFF5F87, 0xFF5FAF, 0xFF5FD7,
    0xFF5FFF, 0xFF8700, 0xFF875F, 0xFF8787, 0xFF87AF, 0xFF87D7, 0xFF87FF, 0xFFAF00, 0xFFAF5F,
    0xFFAF87, 0xFFAFAF, 0xFFAFD7, 0xFFAFFF, 0xFFD700, 0xFFD75F, 0xFFD787, 0xFFD7AF, 0xFFD7D7,
    0xFFD7FF, 0xFFFF00, 0xFFFF5F, 0xFFFF87, 0xFFFFAF, 0xFFFFD7, 0xFFFFFF, 0x080808, 0x121212,
    0x1C1C1C, 0x262626, 0x303030, 0x3A3A3A, 0x444444, 0x4E4E4E, 0x585858, 0x626262, 0x6C6C6C,
    0x767676, 0x808080, 0x8A8A8A, 0x949494, 0x9E9E9E, 0xA8A8A8, 0xB2B2B2, 0xBCBCBC, 0xC6C6C6,
    0xD0D0D0, 0xDADADA, 0xE4E4E4, 0xEEEEEE,
];

const ZERO: u32 = '0' as u32;
const ONE: u32 = '1' as u32;
const TWO: u32 = '2' as u32;
const THREE: u32 = '3' as u32;
const FOUR: u32 = '4' as u32;
const FIVE: u32 = '5' as u32;
const SIX: u32 = '6' as u32;
const EIGHT: u32 = '8' as u32;
const NINE: u32 = '9' as u32;
const SPACE: u32 = ' ' as u32;
const SEMICOLON: u32 = ';' as u32;
const QUESTION_MARK: u32 = '?' as u32;
const LEFT_BRACKET: u32 = '[' as u32;
const RIGHT_BRACKET: u32 = ']' as u32;
const BACK_SLASH: u32 = '\\' as u32;
const LOWERCASE_Q: u32 = 'q' as u32;
const LOWERCASE_L: u32 = 'l' as u32;
const LOWERCASE_H: u32 = 'h' as u32;
const LOWERCASE_M: u32 = 'm' as u32;
const UPPERCASE_A: u32 = 'A' as u32;
const UPPERCASE_B: u32 = 'B' as u32;
const UPPERCASE_C: u32 = 'C' as u32;
const UPPERCASE_D: u32 = 'D' as u32;
const UPPERCASE_H: u32 = 'H' as u32;
const UPPERCASE_J: u32 = 'J' as u32;
const UPPERCASE_K: u32 = 'K' as u32;
const UPPERCASE_X: u32 = 'X' as u32;

const MAX_SCROLLBACK_LINES: usize = 100;
const MIN_GRID_WIDTH: isize = 80;
const MIN_GRID_HEIGHT: isize = 24;

#[cfg(target_os = "windows")]
const SHELLS: &[&str] = &[
    "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
    "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
    "C:\\Windows\\system32\\cmd.exe",
];

#[cfg(target_os = "macos")]
const SHELLS: &[&str] = &["zsh", "bash", "sh"];

pub struct TerminalEmulator {
    pty: Option<Pty>,

    // The position of the terminal's cursor, which follows different rules
    // compared to the document's cursor for compatibility reasons, and may be
    // different from the document's cursor position is the user is selecting text.
    grid_cursor: Position,
    grid_width: isize,
    grid_height: isize,
    grid_line_colors: Vec<Vec<(TerminalHighlightKind, TerminalHighlightKind)>>,

    maintain_cursor_positions: bool,

    is_cursor_visible: bool,
    foreground_color: TerminalHighlightKind,
    background_color: TerminalHighlightKind,
    are_colors_swapped: bool,
    are_colors_bright: bool,
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

            is_cursor_visible: true,
            foreground_color: TerminalHighlightKind::Foreground,
            background_color: TerminalHighlightKind::Background,
            are_colors_swapped: false,
            are_colors_bright: false,
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
        doc: &mut Doc,
        tab: &mut Tab,
        line_pool: &mut LinePool,
        cursor_buffer: &mut TempBuffer<Cursor>,
        time: f32,
        dt: f32,
    ) {
        let Some(mut pty) = self.pty.take() else {
            return;
        };

        self.resize_grid(ui, tab, &mut pty);

        let cursor_buffer = cursor_buffer.get_mut();

        self.maintain_cursor_positions = true;
        self.backup_doc_cursor_positions(doc, cursor_buffer);

        self.expand_doc_to_grid_size(doc, line_pool, time);

        if let Ok(mut output) = pty.output().try_lock() {
            self.handle_control_sequences(ui, doc, tab, &output, line_pool, time);

            output.clear();
        }

        if self.maintain_cursor_positions {
            self.restore_doc_cursor_positions(doc, cursor_buffer);
        }

        self.pty = Some(pty);

        tab.camera.horizontal.reset_velocity();
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
                    let remaining = match output.get(1) {
                        Some(&LEFT_BRACKET) => {
                            self.handle_control_sequences_csi(doc, &output[2..], line_pool, time)
                        }
                        Some(&RIGHT_BRACKET) => Self::handle_control_sequences_osc(&output[2..]),
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
                        }

                        // Otherwise, ignored.
                        Some(&output[1..])
                    }
                    Some(&LOWERCASE_H) => {
                        if parameters.first() == Some(&25) {
                            self.is_cursor_visible = true;
                            self.jump_doc_cursors_to_grid_cursor(doc);
                        }

                        // Otherwise, ignored.
                        Some(&output[1..])
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
                                90 => {
                                    self.foreground_color = TerminalHighlightKind::BrightBackground
                                }
                                91 => self.foreground_color = TerminalHighlightKind::BrightRed,
                                92 => self.foreground_color = TerminalHighlightKind::BrightGreen,
                                93 => self.foreground_color = TerminalHighlightKind::BrightYellow,
                                94 => self.foreground_color = TerminalHighlightKind::BrightBlue,
                                95 => self.foreground_color = TerminalHighlightKind::BrightMagenta,
                                96 => self.foreground_color = TerminalHighlightKind::BrightCyan,
                                97 => {
                                    self.foreground_color = TerminalHighlightKind::BrightForeground
                                }
                                100 => {
                                    self.background_color = TerminalHighlightKind::BrightBackground
                                }
                                101 => self.background_color = TerminalHighlightKind::BrightRed,
                                102 => self.background_color = TerminalHighlightKind::BrightGreen,
                                103 => self.background_color = TerminalHighlightKind::BrightYellow,
                                104 => self.background_color = TerminalHighlightKind::BrightBlue,
                                105 => self.background_color = TerminalHighlightKind::BrightMagenta,
                                106 => self.background_color = TerminalHighlightKind::BrightCyan,
                                107 => {
                                    self.background_color = TerminalHighlightKind::BrightForeground
                                }
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
                        let bounds = match Self::get_parameter(parameters, 0, 0) {
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
                        };

                        if let Some((start, end)) = bounds {
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
            self.grid_line_colors[y as usize].resize(
                self.grid_width as usize,
                (
                    TerminalHighlightKind::Foreground,
                    TerminalHighlightKind::Background,
                ),
            );
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
        doc.insert(start, ['\n'], line_pool, time);

        for _ in 0..self.grid_width {
            let start = doc.end();
            doc.insert(start, [' '], line_pool, time);
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

    fn move_position(&self, position: Position, delta: Position) -> Position {
        self.clamp_position(Position::new(position.x + delta.x, position.y + delta.y))
    }

    fn grid_position_to_doc_position(&self, position: Position, doc: &Doc) -> Position {
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

    fn jump_doc_cursors_to_grid_cursor(&mut self, doc: &mut Doc) {
        if !self.is_cursor_visible {
            return;
        }

        self.maintain_cursor_positions = false;

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

    pub fn pty(&mut self) -> Option<&mut Pty> {
        self.pty.as_mut()
    }
}
