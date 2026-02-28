use std::collections::VecDeque;

use crate::{
    text::syntax_highlighter::TerminalHighlightKind,
    ui::{color::Color, terminal::color_table::COLOR_TABLE},
};

#[derive(Debug)]
pub enum EscapeSequence {
    Plain { len: usize },
    Backspace,
    Tab,
    CarriageReturn,
    Newline,
    ReverseNewline,
    HideCursor,
    ShowCursor,
    SwitchToNormalBuffer,
    SwitchToAlternateBuffer,
    QueryModifyKeyboard,
    QueryModifyCursorKeys,
    QueryModifyFunctionKeys,
    QueryModifyOtherKeys,
    QueryDeviceAttributes,
    ResetFormatting,
    SetColorsBright(bool),
    SetColorsSwapped(bool),
    SetForegroundColor(TerminalHighlightKind),
    SetBackgroundColor(TerminalHighlightKind),
    SetCursorX(usize),
    SetCursorY(usize),
    SetCursorPosition(usize, usize),
    MoveCursorX(isize),
    MoveCursorY(isize),
    MoveCursorYAndResetX(isize),
    ClearToScreenEnd,
    ClearToScreenStart,
    ClearScreen,
    ClearScrollbackLines,
    ClearToLineEnd,
    ClearToLineStart,
    ClearLine,
    InsertLines(usize),
    DeleteLines(usize),
    ScrollUp(usize),
    ScrollDown(usize),
    ClearCharsAfterCursor(usize),
    DeleteCharsAfterCursor(usize),
    SetScrollRegion { top: usize, bottom: usize },
    QueryDeviceStatus,
    QueryTerminalId,
    SetTitle { len: usize },
    ResetTitle,
    QueryForegroundColor,
    QueryBackgroundColor,
}

#[derive(Debug)]
enum EscapeParserState {
    Plain {
        len: usize,
    },
    Escape,
    CsiPrefix,
    CsiParameters {
        prefix: Option<u8>,
        parameters: [usize; 16],
        parameter_index: usize,
    },
    CsiKind {
        prefix: Option<u8>,
        parameters: [usize; 16],
        parameter_count: usize,
    },
    OscParameters {
        parameters: [u8; 16],
        parameter_index: usize,
    },
    OscString {
        parameters: [u8; 16],
        parameter_count: usize,
        len: usize,
    },
    OscTerminator,
    Charset,
    CursorShape,
}

pub struct EscapeParser {
    pending_text: Vec<u8>,
    used_pending_text: usize,
    pending_sequences: VecDeque<EscapeSequence>,
    state: EscapeParserState,
}

impl EscapeParser {
    pub fn new() -> Self {
        Self {
            pending_text: Vec::new(),
            used_pending_text: 0,
            pending_sequences: VecDeque::new(),
            state: EscapeParserState::Plain { len: 0 },
        }
    }

    pub fn next(&mut self, byte: u8) {
        self.pending_text.drain(..self.used_pending_text);
        self.used_pending_text = 0;

        match &mut self.state {
            EscapeParserState::Plain { len, .. } => match byte {
                0x1B => {
                    self.flush();
                    self.state = EscapeParserState::Escape;
                }
                0x7 => {}
                0x8 => {
                    self.flush();
                    self.pending_sequences.push_back(EscapeSequence::Backspace);
                }
                b'\t' => {
                    self.flush();
                    self.pending_sequences.push_back(EscapeSequence::Tab);
                }
                b'\r' => {
                    self.flush();
                    self.pending_sequences
                        .push_back(EscapeSequence::CarriageReturn);
                }
                b'\n' => {
                    self.flush();
                    self.pending_sequences.push_back(EscapeSequence::Newline);
                }
                _ => {
                    self.pending_text.push(byte);
                    *len += 1;
                }
            },
            EscapeParserState::Escape => {
                match byte {
                    b'[' => self.state = EscapeParserState::CsiPrefix,
                    b']' => {
                        self.state = EscapeParserState::OscParameters {
                            parameters: [0; 16],
                            parameter_index: 0,
                        }
                    }
                    b'(' | b')' => self.state = EscapeParserState::Charset,
                    b'M' => {
                        self.pending_sequences
                            .push_back(EscapeSequence::ReverseNewline);
                        self.state = EscapeParserState::Plain { len: 0 };
                    }
                    _ => {
                        #[cfg(feature = "terminal_debug")]
                        println!("Unhandled escape: {}", byte);

                        self.state = EscapeParserState::Plain { len: 0 };
                    }
                };
            }
            EscapeParserState::CsiPrefix => {
                if matches!(byte, b'?' | b'>') {
                    self.state = EscapeParserState::CsiParameters {
                        prefix: Some(byte),
                        parameters: [0; 16],
                        parameter_index: 0,
                    };
                } else {
                    self.state = EscapeParserState::CsiParameters {
                        prefix: None,
                        parameters: [0; 16],
                        parameter_index: 0,
                    };

                    self.next(byte);
                };
            }
            EscapeParserState::CsiParameters {
                prefix,
                parameters,
                parameter_index,
            } => match byte {
                b'0'..=b'9' => {
                    parameters[*parameter_index] =
                        parameters[*parameter_index] * 10 + (byte - b'0') as usize;
                }
                b';' if *parameter_index < 15 => *parameter_index += 1,
                _ => {
                    self.state = EscapeParserState::CsiKind {
                        prefix: *prefix,
                        parameters: *parameters,
                        parameter_count: *parameter_index + 1,
                    };

                    if byte != b';' {
                        self.next(byte);
                    }
                }
            },
            EscapeParserState::CsiKind {
                prefix,
                parameters,
                parameter_count,
            } => {
                let parameters = &parameters[..*parameter_count];

                match *prefix {
                    Some(b'?') => match byte {
                        b'l' => {
                            for parameter in parameters {
                                match parameter {
                                    25 => {
                                        self.pending_sequences.push_back(EscapeSequence::HideCursor)
                                    }
                                    1047 | 1049 => self
                                        .pending_sequences
                                        .push_back(EscapeSequence::SwitchToNormalBuffer),
                                    // Otherwise, ignored.
                                    #[cfg(feature = "terminal_debug")]
                                    parameter => {
                                        println!("Unhandled private mode disabled: {}", parameter)
                                    }
                                    _ => {}
                                }
                            }
                        }
                        b'h' => {
                            for parameter in parameters {
                                match parameter {
                                    25 => {
                                        self.pending_sequences.push_back(EscapeSequence::ShowCursor)
                                    }
                                    1047 | 1049 => self
                                        .pending_sequences
                                        .push_back(EscapeSequence::SwitchToAlternateBuffer),
                                    // Otherwise, ignored.
                                    #[cfg(feature = "terminal_debug")]
                                    parameter => {
                                        println!("Unhandled private mode enabled: {}", parameter)
                                    }
                                    _ => {}
                                }
                            }
                        }
                        b'm' => {
                            for parameter in parameters {
                                let sequence = match parameter {
                                    0 => Some(EscapeSequence::QueryModifyKeyboard),
                                    1 => Some(EscapeSequence::QueryModifyCursorKeys),
                                    2 => Some(EscapeSequence::QueryModifyFunctionKeys),
                                    4 => Some(EscapeSequence::QueryModifyOtherKeys),
                                    _ => None,
                                };

                                if let Some(sequence) = sequence {
                                    self.pending_sequences.push_back(sequence);
                                }
                            }
                        }
                        _ => {}
                    },
                    Some(b'>') => {
                        if byte == b'c' {
                            self.pending_sequences
                                .push_back(EscapeSequence::QueryDeviceAttributes);
                        }
                    }
                    None => match byte {
                        b'm' => {
                            let mut parameters = parameters.iter();

                            // Set text formatting.
                            while let Some(parameter) = parameters.next() {
                                let sequence = match *parameter {
                                    0 => Some(EscapeSequence::ResetFormatting),
                                    1 => Some(EscapeSequence::SetColorsBright(true)),
                                    7 => Some(EscapeSequence::SetColorsSwapped(true)),
                                    22 => Some(EscapeSequence::SetColorsBright(false)),
                                    27 => Some(EscapeSequence::SetColorsSwapped(false)),
                                    30 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::Background,
                                    )),
                                    31 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::Red,
                                    )),
                                    32 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::Green,
                                    )),
                                    33 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::Yellow,
                                    )),
                                    34 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::Blue,
                                    )),
                                    35 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::Magenta,
                                    )),
                                    36 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::Cyan,
                                    )),
                                    37 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::Foreground,
                                    )),
                                    38 => Some(EscapeSequence::SetForegroundColor(
                                        parse_color_from_parameters(&mut parameters)
                                            .unwrap_or(TerminalHighlightKind::Foreground),
                                    )),
                                    39 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::Foreground,
                                    )),
                                    40 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::Background,
                                    )),
                                    41 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::Red,
                                    )),
                                    42 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::Green,
                                    )),
                                    43 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::Yellow,
                                    )),
                                    44 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::Blue,
                                    )),
                                    45 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::Magenta,
                                    )),
                                    46 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::Cyan,
                                    )),
                                    47 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::Foreground,
                                    )),
                                    48 => Some(EscapeSequence::SetBackgroundColor(
                                        parse_color_from_parameters(&mut parameters)
                                            .unwrap_or(TerminalHighlightKind::Background),
                                    )),
                                    49 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::Background,
                                    )),
                                    90 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::BrightBackground,
                                    )),
                                    91 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::BrightRed,
                                    )),
                                    92 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::BrightGreen,
                                    )),
                                    93 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::BrightYellow,
                                    )),
                                    94 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::BrightBlue,
                                    )),
                                    95 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::BrightMagenta,
                                    )),
                                    96 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::BrightCyan,
                                    )),
                                    97 => Some(EscapeSequence::SetForegroundColor(
                                        TerminalHighlightKind::BrightForeground,
                                    )),
                                    100 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::BrightBackground,
                                    )),
                                    101 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::BrightRed,
                                    )),
                                    102 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::BrightGreen,
                                    )),
                                    103 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::BrightYellow,
                                    )),
                                    104 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::BrightBlue,
                                    )),
                                    105 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::BrightMagenta,
                                    )),
                                    106 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::BrightCyan,
                                    )),
                                    107 => Some(EscapeSequence::SetBackgroundColor(
                                        TerminalHighlightKind::BrightForeground,
                                    )),
                                    _ => {
                                        #[cfg(feature = "terminal_debug")]
                                        println!("Unhandled format parameter: {:?}", parameter);

                                        None
                                    }
                                };

                                if let Some(sequence) = sequence {
                                    self.pending_sequences.push_back(sequence);
                                }
                            }
                        }
                        b'l' => {
                            for parameter in parameters {
                                match parameter {
                                    // Otherwise, ignored.
                                    #[cfg(feature = "terminal_debug")]
                                    parameter => {
                                        println!("Unhandled mode disabled: {}", parameter)
                                    }
                                    _ => {}
                                }
                            }
                        }
                        b'h' => {
                            for parameter in parameters {
                                match parameter {
                                    // Otherwise, ignored.
                                    #[cfg(feature = "terminal_debug")]
                                    parameter => {
                                        println!("Unhandled mode enabled: {}", parameter)
                                    }
                                    _ => {}
                                }
                            }
                        }
                        b'G' => {
                            let x = parameter(parameters, 0, 1).saturating_sub(1);
                            self.pending_sequences
                                .push_back(EscapeSequence::SetCursorX(x));
                        }
                        b'd' => {
                            let y = parameter(parameters, 0, 1).saturating_sub(1);
                            self.pending_sequences
                                .push_back(EscapeSequence::SetCursorY(y));
                        }
                        b'H' => {
                            let y = parameter(parameters, 0, 1).saturating_sub(1);
                            let x = parameter(parameters, 1, 1).saturating_sub(1);
                            self.pending_sequences
                                .push_back(EscapeSequence::SetCursorPosition(x, y));
                        }
                        b'A' => {
                            let distance = parameter(parameters, 0, 1) as isize;
                            self.pending_sequences
                                .push_back(EscapeSequence::MoveCursorY(-distance));
                        }
                        b'B' => {
                            let distance = parameter(parameters, 0, 1) as isize;
                            self.pending_sequences
                                .push_back(EscapeSequence::MoveCursorY(distance));
                        }
                        b'C' => {
                            let distance = parameter(parameters, 0, 1) as isize;
                            self.pending_sequences
                                .push_back(EscapeSequence::MoveCursorX(distance));
                        }
                        b'D' => {
                            let distance = parameter(parameters, 0, 1) as isize;
                            self.pending_sequences
                                .push_back(EscapeSequence::MoveCursorX(-distance));
                        }
                        b'E' => {
                            let distance = parameter(parameters, 0, 1) as isize;
                            self.pending_sequences
                                .push_back(EscapeSequence::MoveCursorYAndResetX(distance));
                        }
                        b'F' => {
                            let distance = parameter(parameters, 0, 1) as isize;
                            self.pending_sequences
                                .push_back(EscapeSequence::MoveCursorYAndResetX(-distance));
                        }
                        b'J' => {
                            let sequence = match parameter(parameters, 0, 0) {
                                0 => Some(EscapeSequence::ClearToScreenEnd),
                                1 => Some(EscapeSequence::ClearToScreenStart),
                                2 => Some(EscapeSequence::ClearScreen),
                                3 => Some(EscapeSequence::ClearScrollbackLines),
                                _ => None,
                            };

                            if let Some(sequence) = sequence {
                                self.pending_sequences.push_back(sequence);
                            }
                        }
                        b'K' => {
                            let sequence = match parameter(parameters, 0, 0) {
                                0 => Some(EscapeSequence::ClearToLineEnd),
                                1 => Some(EscapeSequence::ClearToLineStart),
                                2 => Some(EscapeSequence::ClearLine),
                                _ => None,
                            };

                            if let Some(sequence) = sequence {
                                self.pending_sequences.push_back(sequence);
                            }
                        }
                        b'L' => {
                            let count = parameter(parameters, 0, 1);
                            self.pending_sequences
                                .push_back(EscapeSequence::InsertLines(count));
                        }
                        b'M' => {
                            let count = parameter(parameters, 0, 1);
                            self.pending_sequences
                                .push_back(EscapeSequence::DeleteLines(count));
                        }
                        b'S' => {
                            let distance = parameter(parameters, 0, 1);
                            self.pending_sequences
                                .push_back(EscapeSequence::ScrollUp(distance));
                        }
                        b'T' => {
                            let distance = parameter(parameters, 0, 1);
                            self.pending_sequences
                                .push_back(EscapeSequence::ScrollDown(distance));
                        }
                        b'X' => {
                            let distance = parameter(parameters, 0, 1);
                            self.pending_sequences
                                .push_back(EscapeSequence::ClearCharsAfterCursor(distance));
                        }
                        b'P' => {
                            let distance = parameter(parameters, 0, 1);
                            self.pending_sequences
                                .push_back(EscapeSequence::DeleteCharsAfterCursor(distance));
                        }
                        b' ' => {
                            self.state = EscapeParserState::CursorShape;
                            return;
                        }
                        b'r' => {
                            let top = parameter(parameters, 0, 1).saturating_sub(1);
                            let bottom = parameter(parameters, 1, usize::MAX).saturating_sub(1);
                            self.pending_sequences
                                .push_back(EscapeSequence::SetScrollRegion { top, bottom });
                        }
                        b'n' => {
                            for parameter in parameters {
                                let sequence = match *parameter {
                                    6 => Some(EscapeSequence::QueryDeviceStatus),
                                    _ => None,
                                };

                                if let Some(sequence) = sequence {
                                    self.pending_sequences.push_back(sequence);
                                }
                            }
                        }
                        b'c' => self
                            .pending_sequences
                            .push_back(EscapeSequence::QueryTerminalId),
                        _ => {}
                    },
                    _ => {}
                }

                self.state = EscapeParserState::Plain { len: 0 };
            }
            EscapeParserState::OscParameters {
                parameters,
                parameter_index,
            } => {
                if *parameter_index == 16 || byte == b';' {
                    self.state = EscapeParserState::OscString {
                        parameters: *parameters,
                        parameter_count: *parameter_index,
                        len: 0,
                    };

                    if byte != b';' {
                        self.next(byte);
                    }
                } else {
                    parameters[*parameter_index] = byte;
                    *parameter_index += 1;
                }
            }
            EscapeParserState::OscString {
                parameters,
                parameter_count,
                len,
            } => match byte {
                0x7 | 0x1B => {
                    let parameters = &parameters[..*parameter_count];
                    let mut needs_string = false;

                    match parameters {
                        b"0" | b"2" => {
                            if *len > 0 {
                                needs_string = true;

                                self.pending_sequences
                                    .push_back(EscapeSequence::SetTitle { len: *len });
                            } else {
                                self.pending_sequences.push_back(EscapeSequence::ResetTitle);
                            }
                        }
                        // Setting/requesting foreground/background color.
                        // Only requesting the value is supported.
                        b"10" if parameters.get(1) == Some(&b'?') => {
                            self.pending_sequences
                                .push_back(EscapeSequence::QueryForegroundColor);
                        }
                        b"11" if parameters.get(1) == Some(&b'?') => {
                            self.pending_sequences
                                .push_back(EscapeSequence::QueryBackgroundColor);
                        }
                        _ => {}
                    }

                    if !needs_string {
                        self.pending_text.truncate(self.pending_text.len() - *len);
                    }

                    self.state = if byte == 0x1B {
                        EscapeParserState::OscTerminator
                    } else {
                        EscapeParserState::Plain { len: 0 }
                    };
                }
                _ => {
                    self.pending_text.push(byte);
                    *len += 1;
                }
            },
            EscapeParserState::OscTerminator => {
                self.state = EscapeParserState::Plain { len: 0 };

                if byte != b'\\' {
                    self.next(byte);
                }
            }
            EscapeParserState::Charset | EscapeParserState::CursorShape => {
                self.state = EscapeParserState::Plain { len: 0 }
            }
        }
    }

    pub fn flush(&mut self) {
        let EscapeParserState::Plain { len } = self.state else {
            return;
        };

        if len == 0 {
            return;
        }

        self.pending_sequences
            .push_back(EscapeSequence::Plain { len });

        self.state = EscapeParserState::Plain { len: 0 };
    }

    pub fn next_text(&mut self, len: usize) -> &str {
        let start = self.used_pending_text;
        self.used_pending_text += len;

        valid_utf8_range(&self.pending_text[start..self.used_pending_text])
    }

    pub fn next_sequence(&mut self) -> Option<EscapeSequence> {
        self.pending_sequences.pop_front()
    }
}

fn valid_utf8_range(bytes: &[u8]) -> &str {
    match str::from_utf8(bytes) {
        Ok(string) => string,
        Err(err) => unsafe { str::from_utf8_unchecked(&bytes[..err.valid_up_to()]) },
    }
}

fn parse_color_from_parameters<'a>(
    parameters: &mut impl Iterator<Item = &'a usize>,
) -> Option<TerminalHighlightKind> {
    let kind = parameters.next()?;

    match kind {
        2 => {
            // RGB true color:
            let r = (*parameters.next()?).clamp(0, 256);
            let g = (*parameters.next()?).clamp(0, 256);
            let b = (*parameters.next()?).clamp(0, 256);

            Some(TerminalHighlightKind::Custom(Color::from_rgb(
                r as u8, g as u8, b as u8,
            )))
        }
        5 => {
            // 256 color table:
            let index = (*parameters.next()?).clamp(0, COLOR_TABLE.len());

            Some(TerminalHighlightKind::Custom(Color::from_hex(
                COLOR_TABLE[index],
            )))
        }
        _ => None,
    }
}

fn parameter(parameters: &[usize], index: usize, default: usize) -> usize {
    parameters
        .get(index)
        .copied()
        .filter(|parameter| *parameter != 0)
        .unwrap_or(default)
}
