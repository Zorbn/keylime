use std::collections::HashSet;

use serde::Deserialize;

use super::{pattern::Pattern, syntax_highlighter::HighlightKind};

#[derive(Deserialize, Debug)]
pub struct SyntaxToken {
    pub pattern: Pattern,
    pub kind: HighlightKind,
}

#[derive(Deserialize, Debug)]
pub struct SyntaxRange {
    pub start: Pattern,
    pub end: Pattern,
    pub escape: Option<String>,
    pub kind: HighlightKind,
}

#[derive(Debug)]
pub struct Syntax {
    pub keywords: HashSet<String>,
    pub tokens: Vec<SyntaxToken>,
    pub ranges: Vec<SyntaxRange>,
}
