use super::{
    grapheme::{self, GraphemeCursor},
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
            let mut grapheme_cursor = GraphemeCursor::new(0, line.len());

            while grapheme_cursor.cur_cursor() < line.len() {
                let Some((start, end)) = Self::tokenize_identifier(line, &mut grapheme_cursor)
                else {
                    grapheme_cursor.next_boundary(line);
                    continue;
                };

                let token = &line[start..end];

                self.tokens.insert(token);
            }
        }
    }

    pub fn tokenize_identifier(
        line: &str,
        grapheme_cursor: &mut GraphemeCursor,
    ) -> Option<(usize, usize)> {
        let start = grapheme_cursor.cur_cursor();
        let start_grapheme = grapheme::at(start, line);

        if start_grapheme != "_" && !grapheme::is_alphabetic(start_grapheme) {
            return None;
        }

        grapheme_cursor.next_boundary(line);

        while grapheme_cursor.cur_cursor() < line.len() {
            let grapheme = grapheme::at(grapheme_cursor.cur_cursor(), line);

            if grapheme != "_" && !grapheme::is_alphanumeric(grapheme) {
                break;
            }

            grapheme_cursor.next_boundary(line);
        }

        Some((start, grapheme_cursor.cur_cursor()))
    }

    pub fn tokens(&self) -> &Trie {
        &self.tokens
    }
}
