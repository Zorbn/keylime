use crate::text::syntax_highlighter::{HighlightResult, SyntaxHighlighter};

use super::{line_pool::Line, trie::Trie};

pub struct Tokenizer {
    tokens: Trie,
}

impl Tokenizer {
    pub fn new() -> Self {
        Self {
            tokens: Trie::new(),
        }
    }

    pub fn tokenize(&mut self, lines: &[Line]) {
        self.tokens.clear();

        for line in lines {
            let mut x = 0;

            while x < line.len() {
                let HighlightResult::Token { end } = SyntaxHighlighter::match_identifier(line, x)
                else {
                    x += 1;
                    continue;
                };

                let start = x;
                x = end;

                let token_chars = &line[start..end];

                self.tokens.insert(token_chars);
            }
        }
    }

    pub fn tokens(&self) -> &Trie {
        &self.tokens
    }
}
