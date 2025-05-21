use super::grapheme;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GraphemeCategory {
    Identifier,
    Symbol,
    Space,
    Newline,
}

impl GraphemeCategory {
    pub fn new(grapheme: &str) -> Self {
        if grapheme == "\n" {
            Self::Newline
        } else if grapheme::is_whitespace(grapheme) {
            Self::Space
        } else if grapheme == "_" || grapheme::is_alphanumeric(grapheme) {
            Self::Identifier
        } else {
            Self::Symbol
        }
    }
}
