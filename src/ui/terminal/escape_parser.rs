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
}

pub struct EscapeParser {
    pending_text: Vec<u8>,
    pending_text_start: usize,
    pending_sequences: VecDeque<EscapeSequence>,
    state: EscapeParserState,
}

impl EscapeParser {
    pub fn new() -> Self {
        Self {
            pending_text: Vec::new(),
            pending_text_start: 0,
            pending_sequences: VecDeque::new(),
            state: EscapeParserState::Plain { len: 0 },
        }
    }

    pub fn next(&mut self, byte: u8) {
        self.pending_text.drain(..self.pending_text_start);
        self.pending_text_start = 0;

        println!("state: {:?}", self.state);

        match &mut self.state {
            EscapeParserState::Plain { len } => match byte {
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

                // TODO:
                // if let Some(remaining) = remaining {
                //     output = remaining;
                // } else {
                //     #[cfg(feature = "terminal_debug")]
                //     {
                //         print!("Unhandled escape sequence: ");

                //         for c in output.iter().take(8) {
                //             if let Some(c) = char::from_u32(*c as u32) {
                //                 print!("{:?} ", c);
                //             } else {
                //                 print!("{:?} ", c);
                //             }
                //         }

                //         println!();
                //     }

                //     output = &output[1..];
                // }
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
                // println!(
                //     "prefix: {:?}, byte: {:?}, params: {:?}",
                //     prefix, byte, parameters
                // );
                println!("CSI: {:?}", byte);

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
                            println!("SETTING CURSOR: {}, {}", x, y);
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
                            self.state = EscapeParserState::Charset; // TODO: Hack this is actually for setting cursor shape.
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
                // TODO: This skips the last parameter if you actually have 16.
                if *parameter_index == 15 || byte == b';' {
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

                    match parameters {
                        b"0" | b"2" => {
                            if *len > 0 {
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
            EscapeParserState::Charset => self.state = EscapeParserState::Plain { len: 0 },
        }
    }

    pub fn flush(&mut self) {
        let EscapeParserState::Plain { len } = &mut self.state else {
            return;
        };

        if *len == 0 {
            return;
        }

        println!(
            "pushing TEXT: {:?}",
            valid_utf8_range(&self.pending_text[(self.pending_text.len() - *len)..])
        );

        self.pending_sequences
            .push_back(EscapeSequence::Plain { len: *len });

        *len = 0;
    }

    pub fn next_text(&mut self, len: usize) -> &str {
        let start = self.pending_text_start;
        self.pending_text_start += len;

        valid_utf8_range(&self.pending_text[start..self.pending_text_start])
    }

    pub fn next_sequence(&mut self) -> Option<EscapeSequence> {
        self.pending_sequences.pop_front()
    }
}

// pub fn parse_escape_sequences<'a>(mut output: &'a [u8], result: &mut Vec<EscapeSequence<'a>>) {
//     let mut plain_output = output;

//     while !output.is_empty() {
//         let mut reset_plain_output = true;

//         match output[0] {
//             0x1B => {
//                 flush_plain_output(plain_output, output, result);

//                 let remaining = match output.get(1) {
//                     Some(&b'[') => parse_escape_sequences_csi(&output[2..], result),
//                     Some(&b']') => parse_escape_sequences_osc(&output[2..], result),
//                     Some(&b'(') => {
//                         match output.get(2) {
//                             Some(&b'B') => {
//                                 // Use ASCII character set (other character sets are unsupported).
//                                 Some(&output[3..])
//                             }
//                             _ => None,
//                         }
//                     }
//                     Some(&b'=') => {
//                         // Enter alternative keypad mode, ignored.
//                         Some(&output[2..])
//                     }
//                     Some(&b'>') => {
//                         // Exit alternative keypad mode, ignored.
//                         Some(&output[2..])
//                     }
//                     Some(&b'M') => {
//                         result.push(EscapeSequence::ReverseNewline);

//                         Some(&output[2..])
//                     }
//                     _ => None,
//                 };

//                 if let Some(remaining) = remaining {
//                     output = remaining;
//                 } else {
//                     #[cfg(feature = "terminal_debug")]
//                     {
//                         print!("Unhandled escape sequence: ");

//                         for c in output.iter().take(8) {
//                             if let Some(c) = char::from_u32(*c as u32) {
//                                 print!("{:?} ", c);
//                             } else {
//                                 print!("{:?} ", c);
//                             }
//                         }

//                         println!();
//                     }

//                     output = &output[1..];
//                 }
//             }
//             // Bell:
//             0x7 => {
//                 flush_plain_output(plain_output, output, result);

//                 output = &output[1..];
//             }
//             // Backspace:
//             0x8 => {
//                 flush_plain_output(plain_output, output, result);

//                 result.push(EscapeSequence::Backspace);

//                 output = &output[1..];
//             }
//             // Tab:
//             b'\t' => {
//                 flush_plain_output(plain_output, output, result);

//                 result.push(EscapeSequence::Tab);

//                 output = &output[1..];
//             }
//             // Carriage Return:
//             b'\r' => {
//                 flush_plain_output(plain_output, output, result);

//                 result.push(EscapeSequence::CarriageReturn);

//                 output = &output[1..];
//             }
//             // Newline:
//             b'\n' => {
//                 flush_plain_output(plain_output, output, result);

//                 result.push(EscapeSequence::Newline);

//                 output = &output[1..];
//             }
//             _ => {
//                 output = &output[1..];
//                 reset_plain_output = false;
//             }
//         }

//         if reset_plain_output {
//             plain_output = output;
//         }
//     }

//     flush_plain_output(plain_output, output, result);
// }

// fn flush_plain_output<'a>(
//     plain_output: &'a [u8],
//     output: &[u8],
//     result: &mut Vec<EscapeSequence<'a>>,
// ) {
//     let plain_len = output.as_ptr() as usize - plain_output.as_ptr() as usize;

//     if plain_len == 0 {
//         return;
//     }

//     let plain_string = valid_utf8_range(&plain_output[..plain_len]);

//     result.push(EscapeSequence::Plain(plain_string));
// }

fn valid_utf8_range(bytes: &[u8]) -> &str {
    match str::from_utf8(bytes) {
        Ok(string) => string,
        Err(err) => unsafe { str::from_utf8_unchecked(&bytes[..err.valid_up_to()]) },
    }
}

// fn parse_escape_sequences_csi<'a>(
//     mut output: &'a [u8],
//     result: &mut Vec<EscapeSequence>,
// ) -> Option<&'a [u8]> {
//     let mut parameter_buffer = [0; 16];

//     match output.first() {
//         Some(&b'?') => {
//             output = &output[1..];

//             let (output, parameters) = parse_numeric_parameters(output, &mut parameter_buffer);

//             match output.first() {
//                 Some(&b'l') => {
//                     for parameter in parameters {
//                         match parameter {
//                             25 => result.push(EscapeSequence::HideCursor),
//                             1047 | 1049 => result.push(EscapeSequence::SwitchToNormalBuffer),
//                             // Otherwise, ignored.
//                             #[cfg(feature = "terminal_debug")]
//                             parameter => {
//                                 println!("Unhandled private mode disabled: {}", parameter)
//                             }
//                             _ => {}
//                         }
//                     }

//                     Some(&output[1..])
//                 }
//                 Some(&b'h') => {
//                     for parameter in parameters {
//                         match parameter {
//                             25 => result.push(EscapeSequence::ShowCursor),
//                             1047 | 1049 => result.push(EscapeSequence::SwitchToAlternateBuffer),
//                             // Otherwise, ignored.
//                             #[cfg(feature = "terminal_debug")]
//                             parameter => {
//                                 println!("Unhandled private mode enabled: {}", parameter)
//                             }
//                             _ => {}
//                         }
//                     }

//                     Some(&output[1..])
//                 }
//                 Some(&b'm') => {
//                     for parameter in parameters {
//                         let sequence = match parameter {
//                             0 => Some(EscapeSequence::QueryModifyKeyboard),
//                             1 => Some(EscapeSequence::QueryModifyCursorKeys),
//                             2 => Some(EscapeSequence::QueryModifyFunctionKeys),
//                             4 => Some(EscapeSequence::QueryModifyOtherKeys),
//                             _ => return None,
//                         };

//                         if let Some(sequence) = sequence {
//                             result.push(sequence);
//                         }
//                     }

//                     Some(&output[1..])
//                 }
//                 _ => None,
//             }
//         }
//         Some(&b'>') => {
//             output = &output[1..];

//             let (output, _) = parse_numeric_parameters(output, &mut parameter_buffer);

//             match output.first() {
//                 Some(&b'm') => {
//                     // Set/reset xterm modifier key options, ignored.
//                     Some(&output[1..])
//                 }
//                 Some(&b'c') => {
//                     result.push(EscapeSequence::QueryDeviceAttributes);

//                     Some(&output[1..])
//                 }
//                 _ => None,
//             }
//         }
//         _ => parse_unprefixed_escape_sequences_csi(output, result),
//     }
// }

// fn parse_unprefixed_escape_sequences_csi<'a>(
//     mut output: &'a [u8],
//     result: &mut Vec<EscapeSequence>,
// ) -> Option<&'a [u8]> {
//     let mut parameter_buffer = [0; 16];

//     let parameters;
//     (output, parameters) = parse_numeric_parameters(output, &mut parameter_buffer);

//     match output.first() {
//         Some(&b'm') => {
//             let parameters = if parameters.is_empty() {
//                 &[0]
//             } else {
//                 parameters
//             };

//             let mut parameters = parameters.iter();

//             // Set text formatting.
//             while let Some(parameter) = parameters.next() {
//                 let sequence = match *parameter {
//                     0 => Some(EscapeSequence::ResetFormatting),
//                     1 => Some(EscapeSequence::SetColorsBright(true)),
//                     7 => Some(EscapeSequence::SetColorsSwapped(true)),
//                     22 => Some(EscapeSequence::SetColorsBright(false)),
//                     27 => Some(EscapeSequence::SetColorsSwapped(false)),
//                     30 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::Background,
//                     )),
//                     31 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::Red,
//                     )),
//                     32 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::Green,
//                     )),
//                     33 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::Yellow,
//                     )),
//                     34 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::Blue,
//                     )),
//                     35 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::Magenta,
//                     )),
//                     36 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::Cyan,
//                     )),
//                     37 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::Foreground,
//                     )),
//                     38 => {
//                         let color = parse_color_from_parameters(&mut parameters)?;
//                         Some(EscapeSequence::SetForegroundColor(
//                             TerminalHighlightKind::Custom(color),
//                         ))
//                     }
//                     39 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::Foreground,
//                     )),
//                     40 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::Background,
//                     )),
//                     41 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::Red,
//                     )),
//                     42 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::Green,
//                     )),
//                     43 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::Yellow,
//                     )),
//                     44 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::Blue,
//                     )),
//                     45 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::Magenta,
//                     )),
//                     46 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::Cyan,
//                     )),
//                     47 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::Foreground,
//                     )),
//                     48 => {
//                         let color = parse_color_from_parameters(&mut parameters)?;
//                         Some(EscapeSequence::SetBackgroundColor(
//                             TerminalHighlightKind::Custom(color),
//                         ))
//                     }
//                     49 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::Background,
//                     )),
//                     90 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::BrightBackground,
//                     )),
//                     91 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::BrightRed,
//                     )),
//                     92 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::BrightGreen,
//                     )),
//                     93 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::BrightYellow,
//                     )),
//                     94 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::BrightBlue,
//                     )),
//                     95 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::BrightMagenta,
//                     )),
//                     96 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::BrightCyan,
//                     )),
//                     97 => Some(EscapeSequence::SetForegroundColor(
//                         TerminalHighlightKind::BrightForeground,
//                     )),
//                     100 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::BrightBackground,
//                     )),
//                     101 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::BrightRed,
//                     )),
//                     102 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::BrightGreen,
//                     )),
//                     103 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::BrightYellow,
//                     )),
//                     104 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::BrightBlue,
//                     )),
//                     105 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::BrightMagenta,
//                     )),
//                     106 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::BrightCyan,
//                     )),
//                     107 => Some(EscapeSequence::SetBackgroundColor(
//                         TerminalHighlightKind::BrightForeground,
//                     )),
//                     _ => {
//                         #[cfg(feature = "terminal_debug")]
//                         println!("Unhandled format parameter: {:?}", parameter);

//                         None
//                     }
//                 };

//                 if let Some(sequence) = sequence {
//                     result.push(sequence);
//                 }
//             }

//             Some(&output[1..])
//         }
//         Some(&b'l') => {
//             for parameter in parameters {
//                 match parameter {
//                     // Otherwise, ignored.
//                     #[cfg(feature = "terminal_debug")]
//                     parameter => {
//                         println!("Unhandled mode disabled: {}", parameter)
//                     }
//                     _ => {}
//                 }
//             }

//             Some(&output[1..])
//         }
//         Some(&b'h') => {
//             for parameter in parameters {
//                 match parameter {
//                     // Otherwise, ignored.
//                     #[cfg(feature = "terminal_debug")]
//                     parameter => {
//                         println!("Unhandled mode enabled: {}", parameter)
//                     }
//                     _ => {}
//                 }
//             }

//             Some(&output[1..])
//         }
//         Some(&b'G') => {
//             let x = parameter(parameters, 0, 1).saturating_sub(1);

//             result.push(EscapeSequence::SetCursorX(x));

//             Some(&output[1..])
//         }
//         Some(&b'd') => {
//             let y = parameter(parameters, 0, 1).saturating_sub(1);

//             result.push(EscapeSequence::SetCursorY(y));

//             Some(&output[1..])
//         }
//         Some(&b'H') => {
//             let y = parameter(parameters, 0, 1).saturating_sub(1);
//             let x = parameter(parameters, 1, 1).saturating_sub(1);

//             result.push(EscapeSequence::SetCursorPosition(x, y));

//             Some(&output[1..])
//         }
//         Some(&b'A') => {
//             let distance = parameter(parameters, 0, 1) as isize;

//             result.push(EscapeSequence::MoveCursorY(-distance));

//             Some(&output[1..])
//         }
//         Some(&b'B') => {
//             let distance = parameter(parameters, 0, 1) as isize;

//             result.push(EscapeSequence::MoveCursorY(distance));

//             Some(&output[1..])
//         }
//         Some(&b'C') => {
//             let distance = parameter(parameters, 0, 1) as isize;

//             result.push(EscapeSequence::MoveCursorX(distance));

//             Some(&output[1..])
//         }
//         Some(&b'D') => {
//             let distance = parameter(parameters, 0, 1) as isize;

//             result.push(EscapeSequence::MoveCursorX(-distance));

//             Some(&output[1..])
//         }
//         Some(&b'E') => {
//             let distance = parameter(parameters, 0, 1) as isize;

//             result.push(EscapeSequence::MoveCursorYAndResetX(distance));

//             Some(&output[1..])
//         }
//         Some(&b'F') => {
//             let distance = parameter(parameters, 0, 1) as isize;

//             result.push(EscapeSequence::MoveCursorYAndResetX(-distance));

//             Some(&output[1..])
//         }
//         Some(&b'J') => {
//             let sequence = match parameter(parameters, 0, 0) {
//                 0 => Some(EscapeSequence::ClearToScreenEnd),
//                 1 => Some(EscapeSequence::ClearToScreenStart),
//                 2 => Some(EscapeSequence::ClearScreen),
//                 3 => Some(EscapeSequence::ClearScrollbackLines),
//                 _ => None,
//             }?;

//             result.push(sequence);

//             Some(&output[1..])
//         }
//         Some(&b'K') => {
//             let sequence = match parameter(parameters, 0, 0) {
//                 0 => Some(EscapeSequence::ClearToLineEnd),
//                 1 => Some(EscapeSequence::ClearToLineStart),
//                 2 => Some(EscapeSequence::ClearLine),
//                 _ => None,
//             }?;

//             result.push(sequence);

//             Some(&output[1..])
//         }
//         Some(&b'L') => {
//             let count = parameter(parameters, 0, 1);

//             result.push(EscapeSequence::InsertLines(count));

//             Some(&output[1..])
//         }
//         Some(&b'M') => {
//             let count = parameter(parameters, 0, 1);

//             result.push(EscapeSequence::DeleteLines(count));

//             Some(&output[1..])
//         }
//         Some(&b'S') => {
//             let distance = parameter(parameters, 0, 1);

//             result.push(EscapeSequence::ScrollUp(distance));

//             Some(&output[1..])
//         }
//         Some(&b'T') => {
//             let distance = parameter(parameters, 0, 1);

//             result.push(EscapeSequence::ScrollDown(distance));

//             Some(&output[1..])
//         }
//         Some(&b'X') => {
//             let distance = parameter(parameters, 0, 1);

//             result.push(EscapeSequence::ClearCharsAfterCursor(distance));

//             Some(&output[1..])
//         }
//         Some(&b'P') => {
//             let distance = parameter(parameters, 0, 1);

//             result.push(EscapeSequence::DeleteCharsAfterCursor(distance));

//             Some(&output[1..])
//         }
//         Some(&b' ') => {
//             output = &output[1..];

//             // Set cursor shape, ignored.
//             (output.first() == Some(&b'q')).then_some(&output[1..])
//         }
//         Some(&b't') => {
//             // Xterm window controls, ignored.
//             Some(&output[1..])
//         }
//         Some(&b'r') => {
//             // Set scroll region.
//             let top = parameter(parameters, 0, 1).saturating_sub(1);
//             let bottom = parameter(parameters, 1, usize::MAX).saturating_sub(1);

//             result.push(EscapeSequence::SetScrollRegion { top, bottom });

//             Some(&output[1..])
//         }
//         Some(&b'n') => {
//             // Device status report.

//             for parameter in parameters {
//                 let sequence = match *parameter {
//                     6 => Some(EscapeSequence::QueryDeviceStatus),
//                     _ => None,
//                 };

//                 if let Some(sequence) = sequence {
//                     result.push(sequence);
//                 }
//             }

//             Some(&output[1..])
//         }
//         Some(&b'c') => {
//             result.push(EscapeSequence::QueryTerminalId);

//             Some(&output[1..])
//         }
//         _ => None,
//     }
// }

// fn parse_escape_sequences_osc<'a>(
//     output: &'a [u8],
//     result: &mut Vec<EscapeSequence<'a>>,
// ) -> Option<&'a [u8]> {
//     let (mut output, kind) = parse_numeric_parameter(output)?;

//     if !output.starts_with(b";") {
//         return None;
//     }

//     output = &output[1..];

//     match kind {
//         0 | 2 => {
//             let title = consume_terminated_string(&mut output);

//             if let Some(title) = title
//                 .and_then(|title| str::from_utf8(title).ok())
//                 .filter(|title| !title.is_empty())
//             {
//                 result.push(EscapeSequence::SetTitle(title));
//             } else {
//                 result.push(EscapeSequence::ResetTitle);
//             }

//             Some(output)
//         }
//         10 | 11 => {
//             // Setting/requesting foreground/background color.

//             if !output.starts_with(b"?") {
//                 // Only requesting the value is supported.
//                 return None;
//             }

//             output = &output[1..];
//             output = consume_string_terminator(output)?;

//             let sequence = if kind == 10 {
//                 EscapeSequence::QueryForegroundColor
//             } else {
//                 EscapeSequence::QueryBackgroundColor
//             };

//             result.push(sequence);

//             Some(output)
//         }
//         _ => {
//             #[cfg(feature = "terminal_debug")]
//             println!("Unhandled osc sequence: {}", kind);

//             consume_terminated_string(&mut output);

//             Some(output)
//         }
//     }
// }

fn consume_terminated_string<'a>(output: &mut &'a [u8]) -> Option<&'a [u8]> {
    let string_bytes = *output;
    let mut string_len = 0;

    loop {
        if output.is_empty() {
            return None;
        }

        if let Some(remaining) = consume_string_terminator(output) {
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

fn parse_numeric_parameters<'a, 'b>(
    mut output: &'a [u8],
    parameter_buffer: &'b mut [usize; 16],
) -> (&'a [u8], &'b [usize]) {
    let mut parameter_count = 0;

    loop {
        let Some((next_output, parameter)) = parse_numeric_parameter(output) else {
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
