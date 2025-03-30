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
        } else if grapheme.chars().all(|c| c.is_whitespace()) {
            GraphemeCategory::Space
        } else if grapheme.chars().all(|c| c.is_alphanumeric() || c == '_') {
            GraphemeCategory::Identifier
        } else {
            GraphemeCategory::Symbol
        }
    }
}
