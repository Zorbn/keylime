#[derive(Debug, Clone)]
pub struct GraphemeCursor {
    inner: unicode_segmentation::GraphemeCursor,
    len: usize,
}

impl GraphemeCursor {
    pub fn new(offset: usize, len: usize) -> Self {
        Self {
            inner: unicode_segmentation::GraphemeCursor::new(offset, len, true),
            len,
        }
    }

    pub fn next_boundary(&mut self, text: &str) -> Option<usize> {
        let offset = self.inner.cur_cursor();

        if offset < self.len && text.as_bytes()[offset] < 0x7F {
            self.inner.set_cursor(offset + 1);
            return Some(offset + 1);
        }

        match self.inner.next_boundary(text, 0) {
            Ok(Some(end)) => Some(end),
            _ => None,
        }
    }

    pub fn previous_boundary(&mut self, text: &str) -> Option<usize> {
        // TODO: Fast path doesn't work here because multi-byte chars start
        // with a byte above 0x7F but may end with one that looks like ASCII.
        // let offset = self.inner.cur_cursor();

        // if offset > 1 && text.as_bytes()[offset - 1] < 0x7F {
        //     self.inner.set_cursor(offset - 1);
        //     return Some(offset - 1);
        // }

        match self.inner.prev_boundary(text, 0) {
            Ok(Some(end)) => Some(end),
            _ => None,
        }
    }

    pub fn set_cursor(&mut self, offset: usize) {
        self.inner.set_cursor(offset);
    }

    pub fn cur_cursor(&self) -> usize {
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
        let start = self.grapheme_cursor.cur_cursor();

        match self.grapheme_cursor.next_boundary(self.text) {
            Some(end) => Some(&self.text[start..end]),
            _ => None,
        }
    }
}

pub fn at(index: usize, text: &str) -> &str {
    let mut grapheme_cursor = GraphemeCursor::new(index, text.len());
    assert!(grapheme_cursor.next_boundary(text).is_some());

    &text[index..grapheme_cursor.cur_cursor()]
}

pub fn get(index: usize, text: &str) -> Option<&str> {
    let mut grapheme_cursor = GraphemeCursor::new(index, text.len());

    if let Some(end) = grapheme_cursor.next_boundary(text) {
        Some(&text[index..end])
    } else {
        None
    }
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

#[derive(Debug, Clone)]
pub struct CharCursor {
    offset: usize,
    len: usize,
}

impl CharCursor {
    pub fn new(offset: usize, len: usize) -> Self {
        Self { offset, len }
    }

    pub fn next_boundary(&mut self, text: &str) -> Option<usize> {
        if self.offset >= self.len {
            return None;
        }

        self.offset += 1;

        while !text.is_char_boundary(self.offset) {
            self.offset += 1;
        }

        Some(self.offset)
    }

    pub fn previous_boundary(&mut self, text: &str) -> Option<usize> {
        if self.offset == 0 {
            return None;
        }

        self.offset -= 1;

        while !text.is_char_boundary(self.offset) {
            self.offset -= 1;
        }

        Some(self.offset)
    }

    pub fn set_cursor(&mut self, offset: usize) {
        self.offset = offset;
    }

    pub fn cur_cursor(&self) -> usize {
        self.offset
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
}

impl<'a> Iterator for CharIterator<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.char_cursor.cur_cursor();

        match self.char_cursor.next_boundary(self.text) {
            Some(end) => Some(&self.text[start..end]),
            _ => None,
        }
    }
}
