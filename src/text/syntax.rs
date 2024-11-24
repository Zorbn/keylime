use std::collections::HashSet;

use serde::Deserialize;

use super::syntax_highlighter::HighlightKind;

#[derive(Deserialize, Clone, Debug)]
pub struct SyntaxRange {
    pub start: String,
    pub end: String,
    pub escape: Option<char>,
    pub max_length: Option<usize>,
    pub kind: HighlightKind,
}

pub struct Syntax {
    pub keywords: HashSet<Vec<char>>,
    pub prefixes: Vec<(Vec<char>, HighlightKind)>,
    pub ranges: Vec<SyntaxRange>,
}

impl Syntax {
    pub fn new(
        keywords: &[&str],
        prefixes: &[(&str, HighlightKind)],
        ranges: &[SyntaxRange],
    ) -> Self {
        let mut keyword_vecs = HashSet::new();

        for keyword in keywords {
            keyword_vecs.insert(keyword.chars().collect());
        }

        let mut prefix_vecs = Vec::new();

        for (prefix, kind) in prefixes {
            prefix_vecs.push((prefix.chars().collect(), *kind));
        }

        Self {
            keywords: keyword_vecs,
            prefixes: prefix_vecs,
            ranges: ranges.to_vec(),
        }
    }
}
