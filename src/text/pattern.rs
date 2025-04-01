use serde::{de::Error, Deserialize, Deserializer};

use super::grapheme::{self, GraphemeSelection, GraphemeSelector};

#[derive(Debug, Clone, Copy)]
pub struct PatternMatch {
    pub start: GraphemeSelection,
    pub end: GraphemeSelection,
}

#[derive(Debug, Clone, Copy)]
struct PartialPatternMatch {
    capture_start: Option<GraphemeSelection>,
    capture_end: Option<GraphemeSelection>,
    end: GraphemeSelection,
}

impl PartialPatternMatch {
    pub fn combine_with_existing_capture(
        &self,
        capture_start: Option<GraphemeSelection>,
        capture_end: Option<GraphemeSelection>,
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
    Grapheme(GraphemeSelection),
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
    code: String,
    parts: Vec<PatternPart>,
}

impl Pattern {
    pub fn parse(code: String) -> Result<Self, &'static str> {
        let mut parts = Vec::new();

        let mut has_capture_start = false;
        let mut has_capture_end = false;
        let mut is_escaped = false;

        let mut grapheme_selector = GraphemeSelector::new(&code);

        while !grapheme_selector.is_at_end(&code) {
            let grapheme_selection = grapheme_selector.selection();
            let grapheme = grapheme_selection.grapheme(&code);

            if is_escaped {
                is_escaped = false;

                parts.push(PatternPart::Literal(Self::get_escaped_literal(
                    &code,
                    grapheme_selection,
                )));

                grapheme_selector.next_boundary(&code);

                continue;
            }

            match grapheme {
                "%" => is_escaped = true,
                "(" => {
                    if has_capture_start {
                        return Err("only one capture is allowed");
                    }

                    has_capture_start = true;

                    parts.push(PatternPart::CaptureStart);
                }
                ")" => {
                    if !has_capture_start || has_capture_end {
                        return Err("mismatched capture end");
                    }

                    has_capture_end = true;

                    parts.push(PatternPart::CaptureEnd);
                }
                "+" | "*" | "-" | "?" => {
                    let is_after_chars = parts.last().is_some_and(|part| {
                        matches!(part, PatternPart::Literal(..) | PatternPart::Class(..))
                    });

                    if !is_after_chars {
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
                    let (class, is_positive) = Self::parse_class(&code, &mut grapheme_selector)?;

                    parts.push(PatternPart::Class(class, is_positive));
                }
                _ => parts.push(PatternPart::Literal(PatternLiteral::Grapheme(
                    grapheme_selector.selection(),
                ))),
            }

            grapheme_selector.next_boundary(&code);
        }

        if has_capture_start && !has_capture_end {
            return Err("unterminated capture");
        }

        if is_escaped {
            return Err("expected another character after an escape character");
        }

        Ok(Self { code, parts })
    }

    fn parse_class(
        code: &str,
        grapheme_selector: &mut GraphemeSelector,
    ) -> Result<(Vec<PatternLiteral>, bool), &'static str> {
        let mut class = Vec::new();
        let mut is_escaped = false;
        let mut is_positive = true;
        let mut is_first = true;

        while !grapheme_selector.is_at_end(code) {
            let grapheme_selection = grapheme_selector.selection();
            let grapheme = grapheme_selection.grapheme(code);

            if is_first && grapheme == "^" {
                is_positive = false;
            }

            is_first = false;

            if is_escaped {
                is_escaped = false;

                class.push(Self::get_escaped_literal(code, grapheme_selection));

                grapheme_selector.next_boundary(code);

                continue;
            }

            match grapheme {
                "%" => is_escaped = true,
                "]" => {
                    return Ok((class, is_positive));
                }
                _ => class.push(PatternLiteral::Grapheme(grapheme_selection)),
            }

            grapheme_selector.next_boundary(code);
        }

        Err("unterminated class")
    }

    fn get_escaped_literal(code: &str, grapheme_selection: GraphemeSelection) -> PatternLiteral {
        let grapheme = grapheme_selection.grapheme(code);

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
            _ => PatternLiteral::Grapheme(grapheme_selection),
        }
    }

    pub fn match_text(&self, text: &str, start: GraphemeSelection) -> Option<PatternMatch> {
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
        start: GraphemeSelection,
    ) -> Option<PartialPatternMatch> {
        let mut grapheme_selector = GraphemeSelector::with_selection(start, text);

        let mut capture_start = None;
        let mut capture_end = None;

        let mut part_index = 0;

        while part_index < parts.len() {
            let part = &parts[part_index];

            match part {
                PatternPart::CaptureStart => {
                    capture_start = Some(grapheme_selector.selection());
                }
                PatternPart::CaptureEnd => {
                    capture_end = Some(grapheme_selector.selection());
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
                            grapheme_selector.selection(),
                        )
                        .map(|pattern_match| {
                            pattern_match.combine_with_existing_capture(capture_start, capture_end)
                        });
                }
                _ => {
                    if self.match_literal_or_class(text, grapheme_selector.selection(), part) {
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
            end: grapheme_selector.selection(),
        })
    }

    fn match_modifier(
        &self,
        text: &str,
        modifier: &PatternModifier,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        start: GraphemeSelection,
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
        start: GraphemeSelection,
    ) -> Option<PartialPatternMatch> {
        if !self.match_literal_or_class(text, start, next_part) {
            return None;
        }

        let mut grapheme_selector = GraphemeSelector::with_selection(start, text);
        grapheme_selector.next_boundary(text);

        self.match_modifier_zero_or_more_greedy(
            text,
            next_part,
            remaining_parts,
            grapheme_selector.selection(),
        )
    }

    fn match_modifier_zero_or_more_greedy(
        &self,
        text: &str,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        start: GraphemeSelection,
    ) -> Option<PartialPatternMatch> {
        let mut grapheme_selector = GraphemeSelector::with_selection(start, text);
        let mut pattern_match: Option<PartialPatternMatch> = None;

        loop {
            pattern_match = self
                .match_parts(text, remaining_parts, grapheme_selector.selection())
                .or(pattern_match);

            if self.match_literal_or_class(text, grapheme_selector.selection(), next_part) {
                grapheme_selector.next_boundary(text);
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
        start: GraphemeSelection,
    ) -> Option<PartialPatternMatch> {
        let mut grapheme_selector = GraphemeSelector::with_selection(start, text);

        loop {
            if let Some(pattern_match) =
                self.match_parts(text, remaining_parts, grapheme_selector.selection())
            {
                return Some(pattern_match);
            }

            if self.match_literal_or_class(text, grapheme_selector.selection(), next_part) {
                grapheme_selector.next_boundary(text);
            } else {
                return None;
            }
        }
    }

    fn match_modifier_zero_or_one(
        &self,
        text: &str,
        next_part: &PatternPart,
        remaining_parts: &[PatternPart],
        start: GraphemeSelection,
    ) -> Option<PartialPatternMatch> {
        let mut pattern_match = self.match_parts(text, remaining_parts, start);

        if self.match_literal_or_class(text, start, next_part) {
            let mut grapheme_selector = GraphemeSelector::with_selection(start, text);
            grapheme_selector.next_boundary(text);

            pattern_match = self
                .match_parts(text, remaining_parts, grapheme_selector.selection())
                .or(pattern_match);
        }

        pattern_match
    }

    fn match_literal_or_class(
        &self,
        text: &str,
        grapheme_selection: GraphemeSelection,
        part: &PatternPart,
    ) -> bool {
        match part {
            PatternPart::Literal(literal) => {
                let Some(grapheme) = grapheme_selection.get_grapheme(text) else {
                    return false;
                };

                self.match_literal(grapheme, literal)
            }
            PatternPart::Class(literals, is_positive) => {
                let Some(grapheme) = grapheme_selection.get_grapheme(text) else {
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
            PatternLiteral::Grapheme(literal_grapheme_selection) => {
                grapheme == literal_grapheme_selection.grapheme(&self.code)
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
    fn deserialize<D>(deserializer: D) -> Result<Pattern, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;

        Pattern::parse(s).map_err(D::Error::custom)
    }
}
