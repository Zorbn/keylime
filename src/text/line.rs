use std::{
    iter::{Skip, Take},
    ops::{Index, Range, RangeFrom, RangeFull, RangeTo},
};

use unicode_segmentation::{GraphemeCursor, Graphemes, UnicodeSegmentation};

use super::grapheme::GraphemeSelection;

#[derive(Debug, Default)]
pub struct Line {
    text: String,
    len: isize,
}

impl Line {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            len: 0,
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }

    pub fn len(&self) -> isize {
        self.len
    }

    fn get_grapheme_start(&self, i: isize) -> isize {
        let mut cursor = GraphemeCursor::new(0, self.text.len(), true);

        for _ in 0..i {
            assert!(matches!(cursor.next_boundary(&self.text, 0), Ok(Some(_))));
        }

        cursor.cur_cursor() as isize
    }

    fn graphemes(&self, range: Range<isize>) -> Take<Skip<Graphemes>> {
        self.text
            .graphemes(true)
            .skip((range.start).max(0) as usize)
            .take((range.end - range.start).max(0) as usize)
    }

    pub fn remove_graphemes(&mut self, range: Range<isize>) {
        let start = self.get_grapheme_start(range.start) as usize;
        let end = self.get_grapheme_start(range.end) as usize;

        self.text.drain(start..end);
        self.len -= range.len() as isize;
    }

    pub fn truncate_graphemes(&mut self, i: isize) {
        let start = self.get_grapheme_start(i) as usize;

        self.text.truncate(start);
        self.len = self.len.min(i);
    }

    pub fn extend(&mut self, text: &str) {
        self.text.push_str(text);
        self.len += text.graphemes(true).count() as isize;
    }

    pub fn insert_grapheme(&mut self, i: isize, grapheme: &str) {
        let start = self.get_grapheme_start(i);

        self.text.insert_str(start as usize, grapheme);
        self.len += 1;
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

impl Index<GraphemeSelection> for Line {
    type Output = str;

    fn index(&self, grapheme_selection: GraphemeSelection) -> &Self::Output {
        grapheme_selection.grapheme(&self.text)
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
