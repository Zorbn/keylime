#[derive(Debug, Clone)]
pub struct GraphemeCursor {
    inner: unicode_segmentation::GraphemeCursor,
    len: usize,
}

impl GraphemeCursor {
    pub fn new(index: usize, len: usize) -> Self {
        Self {
            inner: unicode_segmentation::GraphemeCursor::new(index, len, true),
            len,
        }
    }

    pub fn next_boundary(&mut self, text: &str) -> Option<usize> {
        let index = self.inner.cur_cursor();

        if index < self.len && text.as_bytes()[index] < 0x7F {
            self.inner.set_cursor(index + 1);
            return Some(index + 1);
        }

        match self.inner.next_boundary(text, 0) {
            Ok(Some(end)) => Some(end),
            _ => None,
        }
    }

    pub fn previous_boundary(&mut self, text: &str) -> Option<usize> {
        match self.inner.prev_boundary(text, 0) {
            Ok(Some(end)) => Some(end),
            _ => None,
        }
    }

    pub fn set_index(&mut self, index: usize) {
        self.inner.set_cursor(index);
    }

    pub fn index(&self) -> usize {
        self.inner.cur_cursor()
    }
}

pub struct GraphemeIterator<'a> {
    text: &'a str,
    grapheme_cursor: GraphemeCursor,
}

impl<'a> GraphemeIterator<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            grapheme_cursor: GraphemeCursor::new(0, text.len()),
        }
    }
}

impl<'a> Iterator for GraphemeIterator<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.grapheme_cursor.index();

        match self.grapheme_cursor.next_boundary(self.text) {
            Some(end) => Some(&self.text[start..end]),
            _ => None,
        }
    }
}

pub fn at(index: usize, text: &str) -> &str {
    let mut grapheme_cursor = GraphemeCursor::new(index, text.len());
    grapheme_cursor.next_boundary(text);

    &text[index..grapheme_cursor.index()]
}

pub fn get(index: usize, text: &str) -> Option<&str> {
    let mut grapheme_cursor = GraphemeCursor::new(index, text.len());

    grapheme_cursor
        .next_boundary(text)
        .map(|end| &text[index..end])
}

pub fn is_whitespace(grapheme: &str) -> bool {
    grapheme.chars().all(char::is_whitespace)
}

pub fn is_alphabetic(grapheme: &str) -> bool {
    grapheme.chars().all(char::is_alphabetic)
}

pub fn is_ascii_digit(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_ascii_digit())
}

pub fn is_lowercase(grapheme: &str) -> bool {
    grapheme.chars().all(char::is_lowercase)
}

pub fn is_ascii_punctuation(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_ascii_punctuation())
}

pub fn is_uppercase(grapheme: &str) -> bool {
    grapheme.chars().all(char::is_uppercase)
}

pub fn is_alphanumeric(grapheme: &str) -> bool {
    grapheme.chars().all(char::is_alphanumeric)
}

pub fn is_ascii_hexdigit(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_ascii_hexdigit())
}

#[derive(Debug, Clone)]
pub struct CharCursor {
    index: usize,
    len: usize,
}

impl CharCursor {
    pub fn new(index: usize, len: usize) -> Self {
        Self { index, len }
    }

    pub fn next_boundary(&mut self, text: &str) -> Option<usize> {
        if self.index >= self.len {
            return None;
        }

        self.index += 1;

        while !text.is_char_boundary(self.index) {
            self.index += 1;
        }

        Some(self.index)
    }

    pub fn previous_boundary(&mut self, text: &str) -> Option<usize> {
        if self.index == 0 {
            return None;
        }

        self.index -= 1;

        while !text.is_char_boundary(self.index) {
            self.index -= 1;
        }

        Some(self.index)
    }

    pub fn set_index(&mut self, index: usize) {
        self.index = index;
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

pub struct CharIterator<'a> {
    text: &'a str,
    char_cursor: CharCursor,
}

impl<'a> CharIterator<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            char_cursor: CharCursor::new(0, text.len()),
        }
    }

    pub fn with_offset(offset: usize, text: &'a str) -> Self {
        Self {
            text,
            char_cursor: CharCursor::new(offset, text.len()),
        }
    }
}

impl<'a> Iterator for CharIterator<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.char_cursor.index();

        match self.char_cursor.next_boundary(self.text) {
            Some(end) => Some(&self.text[start..end]),
            _ => None,
        }
    }
}

impl DoubleEndedIterator for CharIterator<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let end = self.char_cursor.index();

        match self.char_cursor.previous_boundary(self.text) {
            Some(start) => Some(&self.text[start..end]),
            _ => None,
        }
    }
}

pub fn char_at(index: usize, text: &str) -> &str {
    let mut char_cursor = CharCursor::new(index, text.len());
    char_cursor.next_boundary(text);

    &text[index..char_cursor.index()]
}

pub fn char_ending_at(index: usize, text: &str) -> &str {
    let mut char_cursor = CharCursor::new(index, text.len());
    char_cursor.previous_boundary(text);

    &text[char_cursor.index()..index]
}
