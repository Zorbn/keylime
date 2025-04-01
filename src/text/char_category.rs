use super::grapheme;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GraphemeCategory {
    Identifier,
    Symbol,
    Space,
    Newline,
}

impl GraphemeCategory {
    pub fn new(grapheme: &str) -> GraphemeCategory {
        if grapheme == "\n" {
            GraphemeCategory::Newline
        } else if grapheme::is_whitespace(grapheme) {
            GraphemeCategory::Space
        } else if grapheme == "_" || grapheme::is_alphanumeric(grapheme) {
            GraphemeCategory::Identifier
        } else {
            GraphemeCategory::Symbol
        }
    }
}
