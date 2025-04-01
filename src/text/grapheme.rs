use unicode_segmentation::GraphemeCursor;

pub fn at(index: usize, text: &str) -> &str {
    let mut grapheme_cursor = GraphemeCursor::new(index, text.len(), true);
    assert!(matches!(
        grapheme_cursor.next_boundary(text, 0),
        Ok(Some(_))
    ));

    &text[index..grapheme_cursor.cur_cursor()]
}

pub fn get(index: usize, text: &str) -> Option<&str> {
    let mut grapheme_cursor = GraphemeCursor::new(index, text.len(), true);

    if let Ok(Some(end)) = grapheme_cursor.next_boundary(text, 0) {
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
