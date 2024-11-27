use serde::Deserialize;

use crate::{text::syntax_highlighter::HighlightKind, ui::color::Color};

#[derive(Deserialize, Debug)]
pub struct Theme {
    pub normal: Color,
    pub comment: Color,
    pub keyword: Color,
    pub function: Color,
    pub number: Color,
    pub symbol: Color,
    pub string: Color,
    pub meta: Color,
    pub selection: Color,
    pub line_number: Color,
    pub border: Color,
    pub background: Color,
}

impl Theme {
    pub fn highlight_kind_to_color(&self, highlight_kind: HighlightKind) -> Color {
        match highlight_kind {
            HighlightKind::Normal => self.normal,
            HighlightKind::Comment => self.comment,
            HighlightKind::Keyword => self.keyword,
            HighlightKind::Function => self.function,
            HighlightKind::Number => self.number,
            HighlightKind::Symbol => self.symbol,
            HighlightKind::String => self.string,
            HighlightKind::Meta => self.meta,
            HighlightKind::Custom(color) => color,
        }
    }

    pub fn is_dark(&self) -> bool {
        let background_average =
            (self.background.r as usize + self.background.g as usize + self.background.b as usize)
                / 3;

        background_average < 128
    }
}
