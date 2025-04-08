use super::{
    syntax_highlighter::{HighlightResult, SyntaxHighlighter},
    trie::Trie,
};

pub struct Tokenizer {
    tokens: Trie,
}

impl Tokenizer {
    pub fn new() -> Self {
        Self {
            tokens: Trie::new(),
        }
    }

    pub fn tokenize(&mut self, lines: &[String]) {
        self.tokens.clear();

        for line in lines {
            let mut x = 0;

            while x < line.len() {
                let HighlightResult::Token { start, end } =
                    SyntaxHighlighter::match_identifier(line, x)
                else {
                    x += 1;
                    continue;
                };

                x = end;

                let token = &line[start..end];

                self.tokens.insert(token);
            }
        }
    }

    pub fn tokens(&self) -> &Trie {
        &self.tokens
    }
}
