use unicode_segmentation::GraphemeCursor;

// TODO: Should all these grapheme_ functions have no grapheme_ since they're in the grapheme module anyway?
pub fn grapheme_is_char(grapheme: &str, c: char) -> bool {
    let mut grapheme_char_count = 0;

    for grapheme_c in grapheme.chars() {
        grapheme_char_count += 1;

        if grapheme_char_count > 1 || c != grapheme_c {
            return false;
        }
    }

    true
}

// TODO: Make sure this function is used everywhere instead of .chars().all()...
pub fn grapheme_is_whitespace(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_whitespace())
}

pub fn grapheme_is_alphabetic(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_alphabetic())
}

pub fn grapheme_is_ascii_digit(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_ascii_digit())
}

pub fn grapheme_is_lowercase(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_lowercase())
}

pub fn grapheme_is_ascii_punctuation(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_ascii_punctuation())
}

pub fn grapheme_is_uppercase(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_uppercase())
}

pub fn grapheme_is_alphanumeric(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_alphanumeric())
}

pub fn grapheme_is_ascii_hexdigit(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn grapheme_is_control(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_control())
}

#[derive(Debug, Clone)]
pub struct GraphemeSelector {
    index: isize,
    last_offset: usize,
    grapheme_cursor: GraphemeCursor,
}

impl GraphemeSelector {
    pub fn new(offset: usize, text: &str) -> Self {
        let mut grapheme_cursor = GraphemeCursor::new(offset, text.len(), true);
        grapheme_cursor.next_boundary(text, 0);

        Self {
            index: 0,
            last_offset: offset,
            grapheme_cursor,
        }
    }

    pub fn next_boundary(&mut self, text: &str) -> bool {
        if self.is_at_end(text) {
            return true;
        }

        let last_offset = self.grapheme_cursor.cur_cursor();
        let result = self.grapheme_cursor.next_boundary(text, 0);

        self.last_offset = last_offset;
        self.index += 1;

        if let Ok(Some(_)) = result {
            true
        } else {
            // Force the grapheme cursor past the end of the string.
            self.grapheme_cursor.set_cursor(text.len() + 1);

            false
        }
    }

    fn get_byte_offsets(&self) -> (usize, usize) {
        (self.last_offset, self.grapheme_cursor.cur_cursor())
    }

    pub fn grapheme<'a>(&self, text: &'a str) -> &'a str {
        let (start, end) = self.get_byte_offsets();

        &text[start..end]
    }

    pub fn get_grapheme<'a>(&self, text: &'a str) -> Option<&'a str> {
        let (start, end) = self.get_byte_offsets();

        text.get(start..end)
    }

    pub fn range_before<'a>(&self, text: &'a str) -> &'a str {
        &text[..self.last_offset]
    }

    pub fn index(&self) -> isize {
        self.index
    }

    pub fn is_at_end(&self, text: &str) -> bool {
        self.grapheme_cursor.cur_cursor() > text.len()
    }

    pub fn grapheme_range<'a>(
        start: &GraphemeSelector,
        end: &GraphemeSelector,
        text: &'a str,
    ) -> &'a str {
        let (start, _) = start.get_byte_offsets();
        let (end, _) = end.get_byte_offsets();

        &text[start..end]
    }
}
