use serde::Deserialize;

use crate::{
    text::syntax_highlighter::{HighlightKind, TerminalHighlightKind},
    ui::color::Color,
};

#[derive(Deserialize, Debug)]
pub struct TerminalTheme {
    pub background: Color,
    pub foreground: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,

    pub bright_background: Color,
    pub bright_foreground: Color,
    pub bright_red: Color,
    pub bright_green: Color,
    pub bright_yellow: Color,
    pub bright_blue: Color,
    pub bright_magenta: Color,
    pub bright_cyan: Color,
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self {
            background: Color::from_hex(0x0C0C0CFF),
            foreground: Color::from_hex(0xCCCCCCFF),
            red: Color::from_hex(0xC50F1FFF),
            green: Color::from_hex(0x13A10EFF),
            yellow: Color::from_hex(0xC19C00FF),
            blue: Color::from_hex(0x0037DAFF),
            magenta: Color::from_hex(0x881798FF),
            cyan: Color::from_hex(0x3A96DDFF),

            bright_background: Color::from_hex(0x767676FF),
            bright_foreground: Color::from_hex(0xF2F2F2FF),
            bright_red: Color::from_hex(0xE74856FF),
            bright_green: Color::from_hex(0x16C60CFF),
            bright_yellow: Color::from_hex(0xF9F1A5FF),
            bright_blue: Color::from_hex(0x3B78FFFF),
            bright_magenta: Color::from_hex(0xB4009EFF),
            bright_cyan: Color::from_hex(0x61D6D6FF),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Theme {
    pub normal: Color,
    pub identifier: Color,
    pub comment: Color,
    pub keyword: Color,
    pub function: Color,
    pub number: Color,
    pub symbol: Color,
    pub string: Color,
    pub meta: Color,
    pub selection: Color,
    pub line_number: Color,
    pub scroll_bar: Color,
    pub border: Color,
    pub background: Color,
    pub info: Color,
    pub warning: Color,
    pub error: Color,

    pub terminal: TerminalTheme,
}

impl Theme {
    pub fn highlight_kind_to_color(&self, highlight_kind: HighlightKind) -> Color {
        match highlight_kind {
            HighlightKind::Normal => self.normal,
            HighlightKind::Identifier => self.identifier,
            HighlightKind::Comment => self.comment,
            HighlightKind::Keyword => self.keyword,
            HighlightKind::Function => self.function,
            HighlightKind::Number => self.number,
            HighlightKind::Symbol => self.symbol,
            HighlightKind::String => self.string,
            HighlightKind::Meta => self.meta,
            HighlightKind::Terminal(terminal_highlight_kind) => match terminal_highlight_kind {
                TerminalHighlightKind::Foreground => self.terminal.foreground,
                TerminalHighlightKind::Background => self.terminal.background,
                TerminalHighlightKind::Red => self.terminal.red,
                TerminalHighlightKind::Green => self.terminal.green,
                TerminalHighlightKind::Yellow => self.terminal.yellow,
                TerminalHighlightKind::Blue => self.terminal.blue,
                TerminalHighlightKind::Magenta => self.terminal.magenta,
                TerminalHighlightKind::Cyan => self.terminal.cyan,
                TerminalHighlightKind::BrightForeground => self.terminal.bright_foreground,
                TerminalHighlightKind::BrightBackground => self.terminal.bright_background,
                TerminalHighlightKind::BrightRed => self.terminal.bright_red,
                TerminalHighlightKind::BrightGreen => self.terminal.bright_green,
                TerminalHighlightKind::BrightYellow => self.terminal.bright_yellow,
                TerminalHighlightKind::BrightBlue => self.terminal.bright_blue,
                TerminalHighlightKind::BrightMagenta => self.terminal.bright_magenta,
                TerminalHighlightKind::BrightCyan => self.terminal.bright_cyan,
                TerminalHighlightKind::Custom(color) => color,
            },
        }
    }

    pub fn is_dark(&self) -> bool {
        let background_average =
            (self.background.r as usize + self.background.g as usize + self.background.b as usize)
                / 3;

        background_average < 128
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            normal: Color::from_hex(0xCCCCCCFF),
            identifier: Color::from_hex(0x9CDCFEFF),
            comment: Color::from_hex(0x6A9955FF),
            keyword: Color::from_hex(0x569CD6FF),
            function: Color::from_hex(0xDCDCAAFF),
            number: Color::from_hex(0xB5CEA8FF),
            symbol: Color::from_hex(0xCCCCCCFF),
            string: Color::from_hex(0xCE9178FF),
            meta: Color::from_hex(0xC586C0FF),
            selection: Color::from_hex(0x4CADE47F),
            line_number: Color::from_hex(0x6E7681FF),
            scroll_bar: Color::from_hex(0x6E76817F),
            border: Color::from_hex(0x2B2B2BFF),
            background: Color::from_hex(0x1E1E1EFF),
            info: Color::from_hex(0x6E7681FF),
            warning: Color::from_hex(0xC5A82DFF),
            error: Color::from_hex(0xAB311FFF),

            terminal: TerminalTheme::default(),
        }
    }
}
