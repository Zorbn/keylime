use serde::Deserialize;

use crate::text::line_pool::Line;

use super::syntax::{Syntax, SyntaxRange};

#[derive(Deserialize, PartialEq, Eq, Clone, Copy, Debug)]
pub enum HighlightKind {
    Normal,
    Comment,
    Keyword,
    Function,
    Number,
    Symbol,
    String,
    Preprocessor,
}

#[derive(Clone, Copy)]
pub struct Highlight {
    pub start: usize,
    pub end: usize,
    pub kind: HighlightKind,
}

pub enum HighlightResult {
    Token { end: usize },
    Range { end: usize, is_finished: bool },
    None,
}

#[derive(Clone)]
pub struct HighlightedLine {
    highlights: Vec<Highlight>,
    unfinished_range_index: Option<usize>,
}

impl HighlightedLine {
    pub fn highlights(&self) -> &[Highlight] {
        &self.highlights
    }
}

impl HighlightedLine {
    pub fn new() -> Self {
        Self {
            highlights: Vec::new(),
            unfinished_range_index: None,
        }
    }

    pub fn clear(&mut self) {
        self.highlights.clear();
        self.unfinished_range_index = None;
    }

    fn push(&mut self, highlight: Highlight) {
        if let Some(last_highlight) = self.highlights.last_mut() {
            if last_highlight.end == highlight.start && last_highlight.kind == highlight.kind {
                last_highlight.end = highlight.end;
                return;
            }
        }

        self.highlights.push(highlight);
    }
}

pub struct SyntaxHighlighter {
    highlighted_lines: Vec<HighlightedLine>,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        Self {
            highlighted_lines: Vec::new(),
        }
    }

    pub fn match_identifier(line: &Line, start: usize) -> HighlightResult {
        let mut i = start;

        if line[i] != '_' && !line[i].is_alphabetic() {
            return HighlightResult::None;
        }

        i += 1;

        while i < line.len() {
            let c = line[i];

            if c != '_' && !c.is_alphanumeric() {
                break;
            }

            i += 1;
        }

        if i < line.len() && line[i] == '!' {
            i += 1;
        }

        HighlightResult::Token { end: i }
    }

    fn match_predicate(line: &Line, start: usize, predicate: fn(char) -> bool) -> HighlightResult {
        let mut i = start;

        while i < line.len() {
            if !predicate(line[i]) {
                if i == start {
                    return HighlightResult::None;
                }

                break;
            }

            i += 1;
        }

        HighlightResult::Token { end: i }
    }

    fn match_number(line: &Line, start: usize) -> HighlightResult {
        let mut i = start;

        if i + 2 < line.len() && line[i] == '0' {
            let base_indicator = line[i + 1];
            let digits_start = start + 2;

            match base_indicator {
                'x' => return Self::match_predicate(line, digits_start, |c| c.is_ascii_hexdigit()),
                'b' => {
                    return Self::match_predicate(line, digits_start, |c| matches!(c, '0' | '1'))
                }
                _ => {}
            }
        }

        let mut has_digit = false;
        let mut has_dot = false;

        while i < line.len() {
            let c = line[i];

            if c.is_ascii_digit() {
                has_digit = true;
                i += 1;
                continue;
            } else if c == '.' {
                if has_dot || !has_digit {
                    break;
                }

                has_dot = true;
                i += 1;
                continue;
            }

            break;
        }

        if has_digit {
            HighlightResult::Token { end: i }
        } else {
            HighlightResult::None
        }
    }

    fn match_text(line: &Line, start: usize, text: &str) -> bool {
        let mut i = start;

        for c in text.chars() {
            if i >= line.len() || c != line[i] {
                return false;
            }

            i += 1;
        }

        true
    }

    fn match_range(
        line: &Line,
        start: usize,
        range: &SyntaxRange,
        is_in_progress: bool,
    ) -> HighlightResult {
        let mut i = start;

        if !is_in_progress {
            if !Self::match_text(line, i, &range.start) {
                return HighlightResult::None;
            }

            i += range.start.len();
        }

        let mut unescaped_len = 0;
        let max_len = range.max_length.unwrap_or(usize::MAX);

        while i < line.len() {
            if unescaped_len > max_len {
                return HighlightResult::None;
            }

            if Some(line[i]) == range.escape {
                i += 2;
                unescaped_len += 1;
                continue;
            }

            if Self::match_text(line, i, &range.end) {
                return HighlightResult::Range {
                    end: i + range.end.len(),
                    is_finished: true,
                };
            }

            i += 1;
            unescaped_len += 1;
        }

        HighlightResult::Range {
            end: i.min(line.len()),
            is_finished: false,
        }
    }

    pub fn update(&mut self, lines: &[Line], syntax: &Syntax, start_y: usize, end_y: usize) {
        if self.highlighted_lines.len() < lines.len() {
            self.highlighted_lines
                .resize(lines.len(), HighlightedLine::new());
        }

        for y in start_y..=end_y {
            let line = &lines[y];

            self.highlighted_lines[y].clear();

            let mut x = 0;

            if y > 0 {
                let last_highlighted_line = &self.highlighted_lines[y - 1];

                if let Some(unfinished_range_index) = last_highlighted_line.unfinished_range_index {
                    let range = &syntax.ranges[unfinished_range_index];

                    if let HighlightResult::Range { end, is_finished } =
                        Self::match_range(line, x, range, true)
                    {
                        self.highlighted_lines[y].push(Highlight {
                            start: x,
                            end,
                            kind: range.kind,
                        });
                        x = end;

                        if !is_finished {
                            self.highlighted_lines[y].unfinished_range_index =
                                Some(unfinished_range_index);
                        }
                    }
                }
            }

            'tokenize: while x < line.len() {
                if line[x].is_whitespace() {
                    self.highlighted_lines[y].push(Highlight {
                        start: x,
                        end: x + 1,
                        kind: HighlightKind::Normal,
                    });
                    x += 1;

                    continue;
                }

                for (i, range) in syntax.ranges.iter().enumerate() {
                    let HighlightResult::Range { end, is_finished } =
                        Self::match_range(line, x, range, false)
                    else {
                        continue;
                    };

                    self.highlighted_lines[y].push(Highlight {
                        start: x,
                        end,
                        kind: range.kind,
                    });
                    x = end;

                    if !is_finished && range.end != "\n" {
                        self.highlighted_lines[y].unfinished_range_index = Some(i);
                    }

                    continue 'tokenize;
                }

                if let HighlightResult::Token { end } = Self::match_identifier(line, x) {
                    let prefix_kind = self.highlighted_lines[y]
                        .highlights
                        .last()
                        .and_then(|last| syntax.prefixes.get(&line[last.start..last.end]));

                    let kind = if let Some(prefix_kind) = prefix_kind {
                        *prefix_kind
                    } else if syntax.keywords.contains(&line[x..end]) {
                        HighlightKind::Keyword
                    } else if end < line.len() && line[end] == '(' {
                        HighlightKind::Function
                    } else {
                        HighlightKind::Normal
                    };

                    self.highlighted_lines[y].push(Highlight {
                        start: x,
                        end,
                        kind,
                    });
                    x = end;

                    continue;
                }

                if let HighlightResult::Token { end } = Self::match_number(line, x) {
                    self.highlighted_lines[y].push(Highlight {
                        start: x,
                        end,
                        kind: HighlightKind::Number,
                    });
                    x = end;

                    continue;
                }

                self.highlighted_lines[y].push(Highlight {
                    start: x,
                    end: x + 1,
                    kind: HighlightKind::Symbol,
                });
                x += 1;
            }
        }
    }

    pub fn highlighted_lines(&self) -> &[HighlightedLine] {
        &self.highlighted_lines
    }
}
