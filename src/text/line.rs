use std::{
    iter::{Skip, Take},
    ops::{Index, Range, RangeFrom, RangeFull, RangeTo},
};

use unicode_segmentation::{GraphemeCursor, Graphemes, UnicodeSegmentation};

#[derive(Debug, Default)]
pub struct Line {
    text: String,
}

impl Line {
    pub fn new() -> Self {
        Self {
            text: String::new(),
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }

    pub fn len(&self) -> isize {
        // TODO: This could be cached by storing len as an option and setting it to none when the line is modified.
        UnicodeSegmentation::graphemes(self.text.as_str(), true).count() as isize
    }

    fn get_grapheme_start(&self, i: isize) -> isize {
        let mut cursor = GraphemeCursor::new(0, self.text.len(), true);

        for _ in 0..i {
            let _ = cursor.next_boundary(&self.text, 0);
        }

        cursor.cur_cursor() as isize
    }

    fn graphemes(&self, range: Range<isize>) -> Take<Skip<Graphemes>> {
        UnicodeSegmentation::graphemes(self.text.as_str(), true)
            .skip((range.start).max(0) as usize)
            .take((range.end - range.start).max(0) as usize)
    }

    pub fn remove_graphemes(&mut self, range: Range<isize>) {
        let start = self.get_grapheme_start(range.start) as usize;
        let end = self.get_grapheme_start(range.end) as usize;

        self.text.drain(start..end);
    }

    pub fn truncate_graphemes(&mut self, i: isize) {
        let start = self.get_grapheme_start(i) as usize;

        self.text.truncate(start);
    }

    pub fn extend(&mut self, text: &str) {
        self.text.push_str(text);
    }

    pub fn insert_grapheme(&mut self, i: isize, grapheme: &str) {
        let start = self.get_grapheme_start(i);

        self.text.insert_str(start as usize, grapheme);
    }

    pub fn get_start(&self) -> isize {
        let mut start = 0;

        for c in self.text.chars() {
            if !c.is_whitespace() {
                break;
            }

            start += 1;
        }

        start
    }
}

impl Index<isize> for Line {
    type Output = str;

    fn index(&self, i: isize) -> &Self::Output {
        let start = self.get_grapheme_start(i) as usize;
        let mut cursor = GraphemeCursor::new(start, self.text.len(), true);

        let Ok(Some(end)) = cursor.next_boundary(&self.text, 0) else {
            return "";
        };

        &self.text[start..end]
    }
}

impl Index<Range<isize>> for Line {
    type Output = str;

    fn index(&self, range: Range<isize>) -> &Self::Output {
        let start = self.get_grapheme_start(range.start) as usize;
        let end = self.get_grapheme_start(range.end) as usize;

        &self.text[start..end]
    }
}

impl Index<RangeTo<isize>> for Line {
    type Output = str;

    fn index(&self, range: RangeTo<isize>) -> &Self::Output {
        let end = self.get_grapheme_start(range.end) as usize;

        &self.text[0..end]
    }
}

impl Index<RangeFrom<isize>> for Line {
    type Output = str;

    fn index(&self, range: RangeFrom<isize>) -> &Self::Output {
        let start = self.get_grapheme_start(range.start) as usize;

        &self.text[start..]
    }
}

impl Index<RangeFull> for Line {
    type Output = str;

    fn index(&self, _range: RangeFull) -> &Self::Output {
        &self.text[..]
    }
}
