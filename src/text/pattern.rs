use std::{borrow::Cow, str::Chars};

use serde::{de::Error, Deserialize, Deserializer};

#[derive(Debug, Clone, Copy)]
pub struct PatternMatch {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy)]
struct PartialPatternMatch {
    capture_start: Option<usize>,
    capture_end: Option<usize>,
    end: usize,
}

impl PartialPatternMatch {
    pub fn combine_with_existing_capture(
        &self,
        capture_start: Option<usize>,
        capture_end: Option<usize>,
    ) -> Self {
        Self {
            capture_start: capture_start.or(self.capture_start),
            capture_end: capture_end.or(self.capture_end),
            end: self.end,
        }
    }
}

#[derive(Debug)]
enum PatternModifier {
    OneOrMore,        // +
    ZeroOrMoreGreedy, // *
    ZeroOrMoreFrugal, // -
    ZeroOrOne,        // ?
}

#[derive(Debug)]
enum PatternLiteral {
    Char(char),
    Any,              // %.
    Letter,           // %a
    Digit,            // %d
    LowerCaseLetter,  // %l
    Punctuation,      // %p
    Whitespace,       // %s
    UpperCaseLetter,  // %u
    Alphanumeric,     // %w
    HexadecimalDigit, // %x
}

#[derive(Debug)]
enum PatternPart {
    CaptureStart, // (
    CaptureEnd,   // )
    Literal(PatternLiteral),
    Class(Vec<PatternLiteral>), // []
    Modifier(PatternModifier),
}

#[derive(Debug)]
pub struct Pattern {
    parts: Vec<PatternPart>,
}

impl Pattern {
    pub fn parse(code: &str) -> Result<Self, &'static str> {
        let mut parts = Vec::new();

        let mut has_capture_start = false;
        let mut has_capture_end = false;
        let mut is_escaped = false;

        let mut chars = code.chars();

        while let Some(c) = chars.next() {
            if is_escaped {
                is_escaped = false;

                parts.push(PatternPart::Literal(Self::get_escaped_literal(c)));

                continue;
            }

            match c {
                '%' => is_escaped = true,
                '(' => {
                    if has_capture_start {
                        return Err("only one capture is allowed");
                    }

                    has_capture_start = true;

                    parts.push(PatternPart::CaptureStart);
                }
                ')' => {
                    if !has_capture_start || has_capture_end {
                        return Err("mismatched capture end");
                    }

                    has_capture_end = true;

                    parts.push(PatternPart::CaptureEnd);
                }
                '+' | '*' | '-' | '?' => {
                    let is_after_chars = parts.last().is_some_and(|part| {
                        matches!(part, PatternPart::Literal(..) | PatternPart::Class(..))
                    });

                    if !is_after_chars {
                        return Err("modifier must follow a literal or a class");
                    }

                    let modified = parts.pop().unwrap();

                    parts.push(PatternPart::Modifier(match c {
                        '+' => PatternModifier::OneOrMore,
                        '*' => PatternModifier::ZeroOrMoreGreedy,
                        '-' => PatternModifier::ZeroOrMoreFrugal,
                        '?' => PatternModifier::ZeroOrOne,
                        _ => unreachable!(),
                    }));

                    parts.push(modified);
                }
                '[' => {
                    let class = Self::parse_class(&mut chars)?;

                    parts.push(PatternPart::Class(class));
                }
                _ => parts.push(PatternPart::Literal(PatternLiteral::Char(c))),
            }
        }

        if has_capture_start && !has_capture_end {
            return Err("unterminated capture");
        }

        if is_escaped {
            return Err("expected another character after an escape character");
        }

        Ok(Self { parts })
    }

    fn parse_class(chars: &mut Chars<'_>) -> Result<Vec<PatternLiteral>, &'static str> {
        let mut class = Vec::new();
        let mut is_escaped = false;

        for c in chars.by_ref() {
            if is_escaped {
                is_escaped = false;

                class.push(Self::get_escaped_literal(c));

                continue;
            }

            match c {
                '%' => is_escaped = true,
                ']' => {
                    return Ok(class);
                }
                _ => class.push(PatternLiteral::Char(c)),
            }
        }

        Err("unterminated class")
    }

    fn get_escaped_literal(c: char) -> PatternLiteral {
        match c {
            '.' => PatternLiteral::Any,
            'a' => PatternLiteral::Letter,
            'd' => PatternLiteral::Digit,
            'l' => PatternLiteral::LowerCaseLetter,
            'p' => PatternLiteral::Punctuation,
            's' => PatternLiteral::Whitespace,
            'u' => PatternLiteral::UpperCaseLetter,
            'w' => PatternLiteral::Alphanumeric,
            'x' => PatternLiteral::HexadecimalDigit,
            _ => PatternLiteral::Char(c),
        }
    }

    pub fn match_text(&self, text: &[char], start: usize) -> Option<PatternMatch> {
        let partial_pattern_match = Self::match_parts(text, &self.parts, start)?;

        Some(PatternMatch {
            start: partial_pattern_match.capture_start.unwrap_or(start),
            end: partial_pattern_match
                .capture_end
                .unwrap_or(partial_pattern_match.end),
        })
    }

    fn match_parts(
        text: &[char],
        parts: &[PatternPart],
        start: usize,
    ) -> Option<PartialPatternMatch> {
        let mut i = start;

        let mut capture_start = None;
        let mut capture_end = None;

        let mut part_index = 0;

        while part_index < parts.len() {
            let part = &parts[part_index];

            match part {
                PatternPart::CaptureStart => {
                    capture_start = Some(i);
                }
                PatternPart::CaptureEnd => {
                    capture_end = Some(i);
                }
                PatternPart::Modifier(modifier) => {
                    let next_part = &parts[part_index + 1];
                    let remaining_parts = &parts[part_index + 2..];

                    return Self::match_modifier(text, modifier, next_part, remaining_parts, i)
                        .map(|pattern_match| {
                            pattern_match.combine_with_existing_capture(capture_start, capture_end)
                        });
                }
                _ => {
                    if Self::match_literal_or_class(text, i, part) {
                        i += 1;
                    } else {
                        return None;
                    }
                }
            }

            part_index += 1;
        }

        Some(PartialPatternMatch {
            capture_start,
            capture_end,
            end: i,
        })
    }

    fn match_modifier(
        text: &[char],
        modifier: &PatternModifier,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        start: usize,
    ) -> Option<PartialPatternMatch> {
        match modifier {
            PatternModifier::OneOrMore => {
                Self::match_modifier_one_or_more(text, next_part, remaining_parts, start)
            }
            PatternModifier::ZeroOrMoreGreedy => {
                Self::match_modifier_zero_or_more_greedy(text, next_part, remaining_parts, start)
            }
            PatternModifier::ZeroOrMoreFrugal => {
                Self::match_modifier_zero_or_more_frugal(text, next_part, remaining_parts, start)
            }
            PatternModifier::ZeroOrOne => {
                Self::match_modifier_zero_or_one(text, next_part, remaining_parts, start)
            }
        }
    }

    fn match_modifier_one_or_more(
        text: &[char],
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        mut i: usize,
    ) -> Option<PartialPatternMatch> {
        if !Self::match_literal_or_class(text, i, next_part) {
            return None;
        }

        i += 1;

        Self::match_modifier_zero_or_more_greedy(text, next_part, remaining_parts, i)
    }

    fn match_modifier_zero_or_more_greedy(
        text: &[char],
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        mut i: usize,
    ) -> Option<PartialPatternMatch> {
        let mut pattern_match: Option<PartialPatternMatch> = None;

        loop {
            pattern_match = Self::match_parts(text, remaining_parts, i).or(pattern_match);

            if Self::match_literal_or_class(text, i, next_part) {
                i += 1;
            } else {
                break;
            }
        }

        pattern_match
    }

    fn match_modifier_zero_or_more_frugal(
        text: &[char],
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        mut i: usize,
    ) -> Option<PartialPatternMatch> {
        loop {
            if let Some(pattern_match) = Self::match_parts(text, remaining_parts, i) {
                return Some(pattern_match);
            }

            if Self::match_literal_or_class(text, i, next_part) {
                i += 1;
            } else {
                return None;
            }
        }
    }

    fn match_modifier_zero_or_one(
        text: &[char],
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        mut i: usize,
    ) -> Option<PartialPatternMatch> {
        let mut pattern_match = Self::match_parts(text, remaining_parts, i);

        if Self::match_literal_or_class(text, i, next_part) {
            i += 1;

            pattern_match = Self::match_parts(text, remaining_parts, i).or(pattern_match);
        }

        pattern_match
    }

    fn match_literal_or_class(text: &[char], index: usize, part: &PatternPart) -> bool {
        let Some(c) = text.get(index) else {
            return false;
        };

        match part {
            PatternPart::Literal(literal) => Self::match_literal(*c, literal),
            PatternPart::Class(literals) => {
                let mut has_match = false;

                for literal in literals {
                    if Self::match_literal(*c, literal) {
                        has_match = true;
                        break;
                    }
                }

                has_match
            }
            _ => false,
        }
    }

    fn match_literal(c: char, literal: &PatternLiteral) -> bool {
        match literal {
            PatternLiteral::Char(literal_c) => *literal_c == c,
            PatternLiteral::Any => true,
            PatternLiteral::Letter => c.is_alphabetic(),
            PatternLiteral::Digit => c.is_ascii_digit(),
            PatternLiteral::LowerCaseLetter => c.is_lowercase(),
            PatternLiteral::Punctuation => c.is_ascii_punctuation(),
            PatternLiteral::Whitespace => c.is_whitespace(),
            PatternLiteral::UpperCaseLetter => c.is_uppercase(),
            PatternLiteral::Alphanumeric => c.is_alphanumeric(),
            PatternLiteral::HexadecimalDigit => c.is_ascii_hexdigit(),
        }
    }
}

#[derive(Deserialize)]
struct BorrowedStr<'a>(#[serde(borrow)] Cow<'a, str>);

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D>(deserializer: D) -> Result<Pattern, D::Error>
    where
        D: Deserializer<'de>,
    {
        let BorrowedStr(s) = Deserialize::deserialize(deserializer)?;

        Pattern::parse(&s).map_err(D::Error::custom)
    }
}
