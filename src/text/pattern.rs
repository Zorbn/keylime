use std::{borrow::Cow, str::Chars};

use serde::{de::Error, Deserialize, Deserializer};

use super::grapheme::{
    grapheme_is_alphabetic, grapheme_is_alphanumeric, grapheme_is_ascii_digit,
    grapheme_is_ascii_hexdigit, grapheme_is_ascii_punctuation, grapheme_is_char,
    grapheme_is_lowercase, grapheme_is_uppercase, grapheme_is_whitespace, GraphemeSelector,
};

#[derive(Debug, Clone)]
pub struct PatternMatch {
    pub start: GraphemeSelector,
    pub end: GraphemeSelector,
}

#[derive(Debug, Clone)]
struct PartialPatternMatch {
    // TODO: The grapheme selectors stored here and in the syntax highlighting structs don't need to be full GraphemeSelectors, they could be
    // something like GraphemeSelections (or just Graphemes?) that store only (index, last_offset, offset) and then selectors can have a method to set to selection.
    // TODO: When the Grapheme struct is created it should be Copy.
    capture_start: Option<GraphemeSelector>,
    capture_end: Option<GraphemeSelector>,
    end: GraphemeSelector,
}

impl PartialPatternMatch {
    pub fn combine_with_existing_capture(
        &self,
        capture_start: Option<GraphemeSelector>,
        capture_end: Option<GraphemeSelector>,
    ) -> Self {
        Self {
            capture_start: capture_start.or(self.capture_start.clone()),
            capture_end: capture_end.or(self.capture_end.clone()),
            end: self.end.clone(),
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
    Class(Vec<PatternLiteral>, bool), // [] or [^]
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
                    let (class, is_positive) = Self::parse_class(&mut chars)?;

                    parts.push(PatternPart::Class(class, is_positive));
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

    fn parse_class(chars: &mut Chars<'_>) -> Result<(Vec<PatternLiteral>, bool), &'static str> {
        let mut class = Vec::new();
        let mut is_escaped = false;
        let mut is_positive = true;
        let mut is_first = true;

        for c in chars.by_ref() {
            if is_first && c == '^' {
                is_positive = false;
            }

            is_first = false;

            if is_escaped {
                is_escaped = false;

                class.push(Self::get_escaped_literal(c));

                continue;
            }

            match c {
                '%' => is_escaped = true,
                ']' => {
                    return Ok((class, is_positive));
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

    pub fn match_text(&self, text: &str, start: GraphemeSelector) -> Option<PatternMatch> {
        let partial_pattern_match = Self::match_parts(text, &self.parts, start.clone())?;

        Some(PatternMatch {
            start: partial_pattern_match.capture_start.unwrap_or(start),
            end: partial_pattern_match
                .capture_end
                .unwrap_or(partial_pattern_match.end),
        })
    }

    fn match_parts(
        text: &str,
        parts: &[PatternPart],
        mut grapheme_selector: GraphemeSelector,
    ) -> Option<PartialPatternMatch> {
        let mut capture_start = None;
        let mut capture_end = None;

        let mut part_index = 0;

        while part_index < parts.len() {
            let part = &parts[part_index];

            match part {
                PatternPart::CaptureStart => {
                    capture_start = Some(grapheme_selector.clone());
                }
                PatternPart::CaptureEnd => {
                    capture_end = Some(grapheme_selector.clone());
                }
                PatternPart::Modifier(modifier) => {
                    let next_part = &parts[part_index + 1];
                    let remaining_parts = &parts[part_index + 2..];

                    return Self::match_modifier(
                        text,
                        modifier,
                        next_part,
                        remaining_parts,
                        grapheme_selector.clone(),
                    )
                    .map(|pattern_match| {
                        pattern_match.combine_with_existing_capture(capture_start, capture_end)
                    });
                }
                _ => {
                    if Self::match_literal_or_class(text, grapheme_selector.clone(), part) {
                        grapheme_selector.next_boundary(text);
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
            end: grapheme_selector.clone(),
        })
    }

    fn match_modifier(
        text: &str,
        modifier: &PatternModifier,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        start: GraphemeSelector,
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
        text: &str,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        mut grapheme_selector: GraphemeSelector,
    ) -> Option<PartialPatternMatch> {
        if !Self::match_literal_or_class(text, grapheme_selector.clone(), next_part) {
            return None;
        }

        grapheme_selector.next_boundary(text);

        Self::match_modifier_zero_or_more_greedy(
            text,
            next_part,
            remaining_parts,
            grapheme_selector,
        )
    }

    fn match_modifier_zero_or_more_greedy(
        text: &str,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        mut grapheme_selector: GraphemeSelector,
    ) -> Option<PartialPatternMatch> {
        let mut pattern_match: Option<PartialPatternMatch> = None;

        loop {
            pattern_match = Self::match_parts(text, remaining_parts, grapheme_selector.clone())
                .or(pattern_match);

            if Self::match_literal_or_class(text, grapheme_selector.clone(), next_part) {
                grapheme_selector.next_boundary(text);
            } else {
                break;
            }
        }

        pattern_match
    }

    fn match_modifier_zero_or_more_frugal(
        text: &str,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        mut grapheme_selector: GraphemeSelector,
    ) -> Option<PartialPatternMatch> {
        loop {
            if let Some(pattern_match) =
                Self::match_parts(text, remaining_parts, grapheme_selector.clone())
            {
                return Some(pattern_match);
            }

            if Self::match_literal_or_class(text, grapheme_selector.clone(), next_part) {
                grapheme_selector.next_boundary(text);
            } else {
                return None;
            }
        }
    }

    fn match_modifier_zero_or_one(
        text: &str,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        mut grapheme_selector: GraphemeSelector,
    ) -> Option<PartialPatternMatch> {
        let mut pattern_match = Self::match_parts(text, remaining_parts, grapheme_selector.clone());

        if Self::match_literal_or_class(text, grapheme_selector.clone(), next_part) {
            grapheme_selector.next_boundary(text);

            pattern_match =
                Self::match_parts(text, remaining_parts, grapheme_selector).or(pattern_match);
        }

        pattern_match
    }

    fn match_literal_or_class(
        text: &str,
        grapheme_selector: GraphemeSelector,
        part: &PatternPart,
    ) -> bool {
        match part {
            PatternPart::Literal(literal) => {
                let Some(grapheme) = grapheme_selector.get_grapheme(text) else {
                    return false;
                };

                Self::match_literal(grapheme, literal)
            }
            PatternPart::Class(literals, is_positive) => {
                let Some(grapheme) = grapheme_selector.get_grapheme(text) else {
                    return !is_positive;
                };

                let mut has_match = false;

                for literal in literals {
                    if Self::match_literal(grapheme, literal) {
                        has_match = true;
                        break;
                    }
                }

                has_match == *is_positive
            }
            _ => false,
        }
    }

    fn match_literal(grapheme: &str, literal: &PatternLiteral) -> bool {
        match literal {
            PatternLiteral::Char(literal_c) => grapheme_is_char(grapheme, *literal_c), // TODO: Should ::Char be ::Grapheme instead?
            PatternLiteral::Any => true,
            PatternLiteral::Letter => grapheme_is_alphabetic(grapheme),
            PatternLiteral::Digit => grapheme_is_ascii_digit(grapheme),
            PatternLiteral::LowerCaseLetter => grapheme_is_lowercase(grapheme),
            PatternLiteral::Punctuation => grapheme_is_ascii_punctuation(grapheme),
            PatternLiteral::Whitespace => grapheme_is_whitespace(grapheme),
            PatternLiteral::UpperCaseLetter => grapheme_is_uppercase(grapheme),
            PatternLiteral::Alphanumeric => grapheme_is_alphanumeric(grapheme),
            PatternLiteral::HexadecimalDigit => grapheme_is_ascii_hexdigit(grapheme),
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
