#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CharCategory {
    Identifier,
    Symbol,
    Space,
    Newline,
}

impl CharCategory {
    pub fn new(c: char) -> CharCategory {
        if c == '\n' {
            CharCategory::Newline
        } else if c.is_whitespace() {
            CharCategory::Space
        } else if c.is_alphanumeric() || c == '_' {
            CharCategory::Identifier
        } else {
            CharCategory::Symbol
        }
    }
}
