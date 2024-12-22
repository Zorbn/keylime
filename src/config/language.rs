use std::iter;

use serde::Deserialize;

use crate::text::syntax::Syntax;

#[derive(Deserialize, Debug, Default, Clone, Copy)]
#[serde(untagged)]
pub enum IndentWidth {
    #[default]
    Tab,
    Spaces(usize),
}

impl IndentWidth {
    pub fn char_count(self) -> usize {
        match self {
            IndentWidth::Tab => 1,
            IndentWidth::Spaces(indent_width) => indent_width,
        }
    }

    pub fn chars(self) -> impl Iterator<Item = char> {
        let (c, count) = match self {
            IndentWidth::Tab => ('\t', 1),
            IndentWidth::Spaces(indent_width) => (' ', indent_width),
        };

        iter::repeat_n(c, count)
    }
}

pub struct Language {
    pub indent_width: IndentWidth,
    pub syntax: Option<Syntax>,
    pub comment: String,
}
