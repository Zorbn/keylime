use std::cmp::Ordering;

pub fn compare_ignore_ascii_case(a: &str, b: &str) -> Ordering {
    for (a_char, b_char) in a.chars().zip(b.chars()) {
        let a_char = a_char.to_ascii_lowercase();
        let b_char = b_char.to_ascii_lowercase();

        let ordering = a_char.cmp(&b_char);

        if ordering != Ordering::Equal {
            return ordering;
        }
    }

    a.len().cmp(&b.len())
}

pub fn score_fuzzy_match(haystack: &str, needle: &str) -> f32 {
    const AWARD_DISTANCE_FALLOFF: f32 = 0.8;
    const AWARD_MATCH_BONUS: f32 = 1.0;
    const AWARD_MAX_AFTER_MISMATCH: f32 = 1.0;

    let mut score = 0.0;
    let mut next_match_award = AWARD_MATCH_BONUS;

    let mut haystack_chars = haystack.chars();
    let mut needle_chars = needle.chars().peekable();

    while let Some((haystack_char, needle_char)) = haystack_chars.next().zip(needle_chars.peek()) {
        let haystack_char = haystack_char.to_ascii_lowercase();
        let needle_char = needle_char.to_ascii_lowercase();

        if haystack_char == needle_char {
            score += next_match_award;
            next_match_award += AWARD_MATCH_BONUS;

            needle_chars.next();
        } else {
            next_match_award =
                AWARD_MAX_AFTER_MISMATCH.min(next_match_award * AWARD_DISTANCE_FALLOFF);
        }
    }

    score
}
