use crate::{gfx::Color, syntax_highlighter::HighlightKind};

pub struct Theme {
    pub normal: Color,
    pub comment: Color,
    pub keyword: Color,
    pub number: Color,
    pub symbol: Color,
    pub string: Color,
    pub border: Color,
    pub background: Color,
}

impl Theme {
    pub fn highlight_kind_to_color(&self, highlight_kind: HighlightKind) -> Color {
        match highlight_kind {
            HighlightKind::Normal => self.normal,
            HighlightKind::Comment => self.comment,
            HighlightKind::Keyword => self.keyword,
            HighlightKind::Number => self.number,
            HighlightKind::Symbol => self.symbol,
            HighlightKind::String => self.string,
        }
    }
}
