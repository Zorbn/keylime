use unicode_segmentation::GraphemeCursor;

#[derive(Debug, Clone)]
pub struct GraphemeSelector {
    index: isize,
    last_offset: usize,
    grapheme_cursor: GraphemeCursor,
}

impl GraphemeSelector {
    pub fn new(text: &str) -> Self {
        let mut grapheme_cursor = GraphemeCursor::new(0, text.len(), true);
        grapheme_cursor.next_boundary(text, 0);

        Self {
            index: 0,
            last_offset: 0,
            grapheme_cursor,
        }
    }

    pub fn with_selection(selection: GraphemeSelection, text: &str) -> Self {
        let grapheme_cursor = GraphemeCursor::new(selection.offset, text.len(), true);

        Self {
            index: selection.index,
            last_offset: selection.last_offset,
            grapheme_cursor,
        }
    }

    pub fn next_boundary(&mut self, text: &str) {
        if self.is_at_end(text) {
            return;
        }

        let last_offset = self.grapheme_cursor.cur_cursor();
        let result = self.grapheme_cursor.next_boundary(text, 0);

        self.last_offset = last_offset;
        self.index += 1;

        if !matches!(result, Ok(Some(_))) {
            // Force the grapheme cursor past the end of the string.
            self.grapheme_cursor.set_cursor(text.len() + 1);
        };
    }

    pub fn is_at_end(&self, text: &str) -> bool {
        self.grapheme_cursor.cur_cursor() > text.len()
    }

    pub fn selection(&self) -> GraphemeSelection {
        GraphemeSelection {
            index: self.index,
            last_offset: self.last_offset,
            offset: self.grapheme_cursor.cur_cursor(),
        }
    }

    pub fn set_selection(&mut self, selection: GraphemeSelection) {
        self.index = selection.index;
        self.last_offset = selection.last_offset;
        self.grapheme_cursor.set_cursor(selection.offset);
    }

    pub fn grapheme<'a>(&self, text: &'a str) -> &'a str {
        self.selection().grapheme(text)
    }

    pub fn index(&self) -> isize {
        self.index
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GraphemeSelection {
    index: isize,
    last_offset: usize,
    offset: usize,
}

impl GraphemeSelection {
    fn get_byte_offsets(&self) -> (usize, usize) {
        (self.last_offset, self.offset)
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

    pub fn grapheme_range<'a>(
        start: &GraphemeSelection,
        end: &GraphemeSelection,
        text: &'a str,
    ) -> &'a str {
        let (start, _) = start.get_byte_offsets();
        let (end, _) = end.get_byte_offsets();

        &text[start..end]
    }
}

pub fn is_char(grapheme: &str, c: char) -> bool {
    let mut grapheme_char_count = 0;

    for grapheme_c in grapheme.chars() {
        grapheme_char_count += 1;

        if grapheme_char_count > 1 || c != grapheme_c {
            return false;
        }
    }

    true
}

pub fn is_whitespace(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_whitespace())
}

pub fn is_alphabetic(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_alphabetic())
}

pub fn is_ascii_digit(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_ascii_digit())
}

pub fn is_lowercase(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_lowercase())
}

pub fn is_ascii_punctuation(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_ascii_punctuation())
}

pub fn is_uppercase(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_uppercase())
}

pub fn is_alphanumeric(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_alphanumeric())
}

pub fn is_ascii_hexdigit(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_control(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_control())
}
