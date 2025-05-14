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

pub fn score_fuzzy_match(path: &str, input: &str) -> f32 {
    const AWARD_DISTANCE_FALLOFF: f32 = 0.8;
    const AWARD_MATCH_BONUS: f32 = 1.0;
    const AWARD_MAX_AFTER_MISMATCH: f32 = 1.0;

    let mut score = 0.0;
    let mut next_match_award = AWARD_MATCH_BONUS;

    let mut path_chars = path.chars();
    let mut input_chars = input.chars().peekable();

    while let Some((path_char, input_char)) = path_chars.next().zip(input_chars.peek()) {
        let path_char = path_char.to_ascii_lowercase();
        let input_char = input_char.to_ascii_lowercase();

        if path_char == input_char {
            score += next_match_award;
            next_match_award += AWARD_MATCH_BONUS;

            input_chars.next();
        } else {
            next_match_award =
                AWARD_MAX_AFTER_MISMATCH.min(next_match_award * AWARD_DISTANCE_FALLOFF);
        }
    }

    score
}
