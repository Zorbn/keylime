use std::collections::HashSet;

use crate::text::syntax_highlighter::{HighlightResult, SyntaxHighlighter};

use super::line_pool::{Line, LinePool};

pub struct Tokenizer {
    tokens: HashSet<Line>,
    token_pool: LinePool,
}

impl Tokenizer {
    pub fn new() -> Self {
        Self {
            tokens: HashSet::new(),
            token_pool: LinePool::new(),
        }
    }

    pub fn tokenize(&mut self, lines: &[Line]) {
        for token in self.tokens.drain() {
            self.token_pool.push(token);
        }

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

                if !self.tokens.contains(token_chars) {
                    let mut token = self.token_pool.pop();
                    token.extend_from_slice(token_chars);

                    self.tokens.insert(token);
                }
            }
        }
    }

    pub fn tokens(&self) -> &HashSet<Line> {
        &self.tokens
    }
}
