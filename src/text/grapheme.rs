// TODO: Make sure this function is used everywhere instead of .chars().all()...
pub fn grapheme_is_whitespace(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_whitespace())
}

pub fn grapheme_is_control(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_control())
}
