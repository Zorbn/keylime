use serde::{de::Error, Deserialize, Deserializer};

use crate::pool::Pooled;

use super::grapheme::{self, GraphemeCursor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, PartialEq)]
enum PatternModifier {
    OneOrMore,        // +
    ZeroOrMoreGreedy, // *
    ZeroOrMoreFrugal, // -
    ZeroOrOne,        // ?
}

#[derive(Debug, PartialEq)]
enum PatternLiteral {
    Grapheme(usize, usize),
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

#[derive(Debug, PartialEq)]
enum PatternPart {
    TextStart,    // ^
    CaptureStart, // (
    CaptureEnd,   // )
    Literal(PatternLiteral),
    Class(Vec<PatternLiteral>, bool), // [] or [^]
    Modifier(PatternModifier),
}

#[derive(Debug, PartialEq)]
pub struct Pattern {
    code: Pooled<String>,
    parts: Vec<PatternPart>,
}

impl Pattern {
    pub fn parse(code: Pooled<String>) -> Result<Self, &'static str> {
        let mut parts = Vec::new();

        let mut capture_start = None;
        let mut capture_end = None;
        let mut is_escaped = false;

        let mut grapheme_cursor = GraphemeCursor::new(0, code.len());

        while grapheme_cursor.index() < code.len() {
            let index = grapheme_cursor.index();

            if is_escaped {
                is_escaped = false;

                parts.push(PatternPart::Literal(Self::escaped_literal(&code, index)));

                grapheme_cursor.next_boundary(&code);

                continue;
            }

            let grapheme = grapheme::at(index, &code);

            match grapheme {
                "%" => is_escaped = true,
                "^" => parts.push(PatternPart::TextStart),
                "(" => {
                    if capture_start.is_some() {
                        return Err("only one capture is allowed");
                    }

                    capture_start = Some(parts.len());

                    parts.push(PatternPart::CaptureStart);
                }
                ")" => {
                    if capture_start.is_none() || capture_end.is_some() {
                        return Err("mismatched capture end");
                    }

                    capture_end = Some(parts.len());

                    parts.push(PatternPart::CaptureEnd);
                }
                "+" | "*" | "-" | "?" => {
                    let is_suffix = parts.last().is_some_and(|part| {
                        matches!(part, PatternPart::Literal(..) | PatternPart::Class(..))
                    });

                    if !is_suffix {
                        return Err("modifier must follow a literal or a class");
                    }

                    let modified = parts.pop().unwrap();

                    parts.push(PatternPart::Modifier(match grapheme {
                        "+" => PatternModifier::OneOrMore,
                        "*" => PatternModifier::ZeroOrMoreGreedy,
                        "-" => PatternModifier::ZeroOrMoreFrugal,
                        "?" => PatternModifier::ZeroOrOne,
                        _ => unreachable!(),
                    }));

                    parts.push(modified);
                }
                "[" => {
                    let (class, is_positive) = Self::parse_class(&code, &mut grapheme_cursor)?;

                    parts.push(PatternPart::Class(class, is_positive));
                }
                _ => parts.push(PatternPart::Literal(PatternLiteral::Grapheme(
                    index,
                    index + grapheme.len(),
                ))),
            }

            grapheme_cursor.next_boundary(&code);
        }

        if capture_start.is_some() && capture_end.is_none() {
            return Err("unterminated capture");
        }

        if is_escaped {
            return Err("expected another character after an escape character");
        }

        Self::ensure_captures_something(&parts, capture_start, capture_end)?;

        Ok(Self { code, parts })
    }

    fn ensure_captures_something(
        parts: &[PatternPart],
        capture_start: Option<usize>,
        capture_end: Option<usize>,
    ) -> Result<(), &'static str> {
        let capture_start = capture_start.unwrap_or_default();
        let capture_end = capture_end.unwrap_or(parts.len());

        let mut capture_parts = parts[capture_start..capture_end].iter();
        let mut will_capture_nothing = true;

        while let Some(part) = capture_parts.next() {
            match part {
                PatternPart::Literal(..) | PatternPart::Class(..) => {
                    will_capture_nothing = false;
                    break;
                }
                PatternPart::Modifier(
                    PatternModifier::ZeroOrMoreFrugal
                    | PatternModifier::ZeroOrMoreGreedy
                    | PatternModifier::ZeroOrOne,
                ) => {
                    capture_parts.next();
                }
                _ => {}
            }
        }

        if will_capture_nothing {
            Err("may capture nothing")
        } else {
            Ok(())
        }
    }

    fn parse_class(
        code: &str,
        grapheme_cursor: &mut GraphemeCursor,
    ) -> Result<(Vec<PatternLiteral>, bool), &'static str> {
        // Skip opening bracket.
        grapheme_cursor.next_boundary(code);

        let mut class = Vec::new();
        let mut is_escaped = false;
        let mut is_positive = true;
        let mut is_first = true;

        while grapheme_cursor.index() < code.len() {
            let index = grapheme_cursor.index();
            let grapheme = grapheme::at(index, code);

            if is_escaped {
                is_escaped = false;

                class.push(Self::escaped_literal(code, index));

                grapheme_cursor.next_boundary(code);

                continue;
            }

            match grapheme {
                "^" if is_first => is_positive = false,
                "%" => is_escaped = true,
                "]" => {
                    return Ok((class, is_positive));
                }
                _ => class.push(PatternLiteral::Grapheme(index, index + grapheme.len())),
            }

            is_first = false;
            grapheme_cursor.next_boundary(code);
        }

        Err("unterminated class")
    }

    fn escaped_literal(code: &str, start: usize) -> PatternLiteral {
        let grapheme = grapheme::at(start, code);

        match grapheme {
            "." => PatternLiteral::Any,
            "a" => PatternLiteral::Letter,
            "d" => PatternLiteral::Digit,
            "l" => PatternLiteral::LowerCaseLetter,
            "p" => PatternLiteral::Punctuation,
            "s" => PatternLiteral::Whitespace,
            "u" => PatternLiteral::UpperCaseLetter,
            "w" => PatternLiteral::Alphanumeric,
            "x" => PatternLiteral::HexadecimalDigit,
            _ => PatternLiteral::Grapheme(start, start + grapheme.len()),
        }
    }

    pub fn match_text(&self, text: &str, start: usize) -> Option<PatternMatch> {
        let partial_pattern_match = self.match_parts(text, &self.parts, start)?;

        Some(PatternMatch {
            start: partial_pattern_match.capture_start.unwrap_or(start),
            end: partial_pattern_match
                .capture_end
                .unwrap_or(partial_pattern_match.end),
        })
    }

    fn match_parts(
        &self,
        text: &str,
        parts: &[PatternPart],
        start: usize,
    ) -> Option<PartialPatternMatch> {
        let mut grapheme_cursor = GraphemeCursor::new(start, text.len());

        let mut capture_start = None;
        let mut capture_end = None;

        let mut part_index = 0;

        while part_index < parts.len() {
            let part = &parts[part_index];

            match part {
                PatternPart::TextStart => {
                    if grapheme_cursor.index() != 0 {
                        return None;
                    }
                }
                PatternPart::CaptureStart => {
                    capture_start = Some(grapheme_cursor.index());
                }
                PatternPart::CaptureEnd => {
                    capture_end = Some(grapheme_cursor.index());
                }
                PatternPart::Modifier(modifier) => {
                    let next_part = &parts[part_index + 1];
                    let remaining_parts = &parts[part_index + 2..];

                    return self
                        .match_modifier(
                            text,
                            modifier,
                            next_part,
                            remaining_parts,
                            grapheme_cursor.index(),
                        )
                        .map(|pattern_match| {
                            pattern_match.combine_with_existing_capture(capture_start, capture_end)
                        });
                }
                _ => {
                    if self.match_literal_or_class(text, grapheme_cursor.index(), part) {
                        grapheme_cursor.next_boundary(text);
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
            end: grapheme_cursor.index(),
        })
    }

    fn match_modifier(
        &self,
        text: &str,
        modifier: &PatternModifier,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        start: usize,
    ) -> Option<PartialPatternMatch> {
        match modifier {
            PatternModifier::OneOrMore => {
                self.match_modifier_one_or_more(text, next_part, remaining_parts, start)
            }
            PatternModifier::ZeroOrMoreGreedy => {
                self.match_modifier_zero_or_more_greedy(text, next_part, remaining_parts, start)
            }
            PatternModifier::ZeroOrMoreFrugal => {
                self.match_modifier_zero_or_more_frugal(text, next_part, remaining_parts, start)
            }
            PatternModifier::ZeroOrOne => {
                self.match_modifier_zero_or_one(text, next_part, remaining_parts, start)
            }
        }
    }

    fn match_modifier_one_or_more(
        &self,
        text: &str,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        start: usize,
    ) -> Option<PartialPatternMatch> {
        let mut grapheme_cursor = GraphemeCursor::new(start, text.len());

        if !self.match_literal_or_class(text, grapheme_cursor.index(), next_part) {
            return None;
        }

        grapheme_cursor.next_boundary(text);

        self.match_modifier_zero_or_more_greedy(
            text,
            next_part,
            remaining_parts,
            grapheme_cursor.index(),
        )
    }

    fn match_modifier_zero_or_more_greedy(
        &self,
        text: &str,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        start: usize,
    ) -> Option<PartialPatternMatch> {
        let mut grapheme_cursor = GraphemeCursor::new(start, text.len());
        let mut pattern_match: Option<PartialPatternMatch> = None;

        loop {
            pattern_match = self
                .match_parts(text, remaining_parts, grapheme_cursor.index())
                .or(pattern_match);

            if grapheme_cursor.index() >= text.len() {
                break;
            }

            if self.match_literal_or_class(text, grapheme_cursor.index(), next_part) {
                grapheme_cursor.next_boundary(text);
            } else {
                break;
            }
        }

        pattern_match
    }

    fn match_modifier_zero_or_more_frugal(
        &self,
        text: &str,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        start: usize,
    ) -> Option<PartialPatternMatch> {
        let mut grapheme_cursor = GraphemeCursor::new(start, text.len());

        loop {
            if let Some(pattern_match) =
                self.match_parts(text, remaining_parts, grapheme_cursor.index())
            {
                return Some(pattern_match);
            }

            if grapheme_cursor.index() >= text.len() {
                break;
            }

            if self.match_literal_or_class(text, grapheme_cursor.index(), next_part) {
                grapheme_cursor.next_boundary(text);
            } else {
                break;
            }
        }

        None
    }

    fn match_modifier_zero_or_one(
        &self,
        text: &str,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        start: usize,
    ) -> Option<PartialPatternMatch> {
        let mut pattern_match = self.match_parts(text, remaining_parts, start);
        let mut grapheme_cursor = GraphemeCursor::new(start, text.len());

        if self.match_literal_or_class(text, grapheme_cursor.index(), next_part) {
            grapheme_cursor.next_boundary(text);

            pattern_match = self
                .match_parts(text, remaining_parts, grapheme_cursor.index())
                .or(pattern_match);
        }

        pattern_match
    }

    fn match_literal_or_class(&self, text: &str, start: usize, part: &PatternPart) -> bool {
        match part {
            PatternPart::Literal(literal) => {
                let Some(grapheme) = grapheme::get(start, text) else {
                    return false;
                };

                self.match_literal(grapheme, literal)
            }
            PatternPart::Class(literals, is_positive) => {
                let Some(grapheme) = grapheme::get(start, text) else {
                    return !is_positive;
                };

                let mut has_match = false;

                for literal in literals {
                    if self.match_literal(grapheme, literal) {
                        has_match = true;
                        break;
                    }
                }

                has_match == *is_positive
            }
            _ => false,
        }
    }

    fn match_literal(&self, grapheme: &str, literal: &PatternLiteral) -> bool {
        match literal {
            PatternLiteral::Grapheme(literal_grapheme_start, literal_grapheme_end) => {
                grapheme == &self.code[*literal_grapheme_start..*literal_grapheme_end]
            }
            PatternLiteral::Any => true,
            PatternLiteral::Letter => grapheme::is_alphabetic(grapheme),
            PatternLiteral::Digit => grapheme::is_ascii_digit(grapheme),
            PatternLiteral::LowerCaseLetter => grapheme::is_lowercase(grapheme),
            PatternLiteral::Punctuation => grapheme::is_ascii_punctuation(grapheme),
            PatternLiteral::Whitespace => grapheme::is_whitespace(grapheme),
            PatternLiteral::UpperCaseLetter => grapheme::is_uppercase(grapheme),
            PatternLiteral::Alphanumeric => grapheme::is_alphanumeric(grapheme),
            PatternLiteral::HexadecimalDigit => grapheme::is_ascii_hexdigit(grapheme),
        }
    }
}

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Pooled<String> = Deserialize::deserialize(deserializer)?;

        Self::parse(s).map_err(D::Error::custom)
    }
}
