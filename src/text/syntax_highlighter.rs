use std::ops::RangeInclusive;

use serde::Deserialize;

use crate::{pool::Pooled, ui::color::Color};

use super::{
    grapheme::{self, GraphemeCursor},
    syntax::{Syntax, SyntaxRange, SyntaxToken},
    tokenizer::Tokenizer,
};

#[derive(Deserialize, PartialEq, Eq, Clone, Copy, Debug)]
pub enum TerminalHighlightKind {
    Foreground,
    Background,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,

    BrightForeground,
    BrightBackground,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,

    Custom(Color),
}

#[derive(Deserialize, PartialEq, Eq, Clone, Copy, Debug)]
pub enum HighlightKind {
    Normal,
    Identifier,
    Comment,
    Keyword,
    Function,
    Number,
    Symbol,
    String,
    Meta,
    Terminal(TerminalHighlightKind),
}

#[derive(Debug, Clone)]
pub struct Highlight {
    pub start: usize,
    pub end: usize,
    pub foreground: HighlightKind,
    pub background: Option<HighlightKind>,
}

pub enum HighlightResult {
    Token {
        start: usize,
        end: usize,
    },
    Range {
        start: usize,
        end: usize,
        is_finished: bool,
    },
    None,
}

#[derive(Clone)]
pub struct HighlightedLine {
    highlights: Vec<Highlight>,
    unfinished_range_index: Option<usize>,
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
        if highlight.start == highlight.end {
            return;
        }

        if let Some(last_highlight) = self.highlights.last_mut() {
            if last_highlight.end == highlight.start
                && last_highlight.foreground == highlight.foreground
                && last_highlight.background == highlight.background
            {
                last_highlight.end = highlight.end;
                return;
            }
        }

        self.highlights.push(highlight);
    }

    pub fn highlights(&self) -> &[Highlight] {
        &self.highlights
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

    pub fn clear(&mut self) {
        for highlighted_line in &mut self.highlighted_lines {
            highlighted_line.clear();
        }
    }

    pub fn match_identifier(line: &str, start: usize) -> HighlightResult {
        let mut grapheme_cursor = GraphemeCursor::new(start, line.len());

        if let Some((start, end)) = Tokenizer::tokenize_identifier(line, &mut grapheme_cursor) {
            HighlightResult::Token { start, end }
        } else {
            HighlightResult::None
        }
    }

    fn match_token(line: &str, start: usize, token: &SyntaxToken) -> HighlightResult {
        match token.pattern.match_text(line, start) {
            Some(pattern_match) => HighlightResult::Token {
                start: pattern_match.start,
                end: pattern_match.end,
            },
            None => HighlightResult::None,
        }
    }

    fn match_range(
        line: &str,
        start: usize,
        range: &SyntaxRange,
        is_in_progress: bool,
    ) -> HighlightResult {
        let mut grapheme_cursor = GraphemeCursor::new(start, line.len());

        if !is_in_progress {
            match range.start.match_text(line, start) {
                Some(pattern_match) => grapheme_cursor.set_index(pattern_match.end),
                None => return HighlightResult::None,
            }
        }

        while grapheme_cursor.index() < line.len() {
            let index = grapheme_cursor.index();

            if Some(grapheme::at(index, line)) == range.escape.as_deref() {
                grapheme_cursor.next_boundary(line);
                grapheme_cursor.next_boundary(line);
                continue;
            }

            if let Some(pattern_match) = range.end.match_text(line, index) {
                return HighlightResult::Range {
                    start,
                    end: pattern_match.end,
                    is_finished: true,
                };
            }

            grapheme_cursor.next_boundary(line);
        }

        HighlightResult::Range {
            start,
            end: grapheme_cursor.index(),
            is_finished: false,
        }
    }

    pub fn update(
        &mut self,
        lines: &[Pooled<String>],
        syntax: &Syntax,
        start_y: usize,
        end_y: usize,
    ) {
        if self.highlighted_lines.len() < lines.len() {
            self.highlighted_lines
                .resize(lines.len(), HighlightedLine::new());
        }

        for (y, line) in lines.iter().enumerate().take(end_y + 1).skip(start_y) {
            self.highlighted_lines[y].clear();

            let mut grapheme_cursor = GraphemeCursor::new(0, line.len());

            if y > 0 {
                let last_highlighted_line = &self.highlighted_lines[y - 1];

                if let Some(unfinished_range_index) = last_highlighted_line.unfinished_range_index {
                    let range = &syntax.ranges[unfinished_range_index];

                    if let HighlightResult::Range {
                        start,
                        end,
                        is_finished,
                    } = Self::match_range(line, grapheme_cursor.index(), range, true)
                    {
                        self.highlighted_lines[y].push(Highlight {
                            start,
                            end,
                            foreground: range.kind,
                            background: None,
                        });
                        grapheme_cursor.set_index(end);

                        if !is_finished {
                            self.highlighted_lines[y].unfinished_range_index =
                                Some(unfinished_range_index);
                        }
                    }
                }
            }

            self.highlight_line(line, syntax, &mut grapheme_cursor, y);
        }
    }

    fn highlight_line(
        &mut self,
        line: &str,
        syntax: &Syntax,
        grapheme_cursor: &mut GraphemeCursor,
        y: usize,
    ) {
        'tokenize: while grapheme_cursor.index() < line.len() {
            let default_start = grapheme_cursor.index();
            grapheme_cursor.next_boundary(line);

            let mut default_end = grapheme_cursor.index();
            grapheme_cursor.set_index(default_start);

            let mut default_foreground = HighlightKind::Normal;

            if !grapheme::is_whitespace(grapheme::at(grapheme_cursor.index(), line)) {
                if let HighlightResult::Token { start, end } =
                    Self::match_identifier(line, grapheme_cursor.index())
                {
                    let identifier = &line[start..end];

                    if syntax.keywords.contains(identifier) {
                        self.highlighted_lines[y].push(Highlight {
                            start,
                            end,
                            foreground: HighlightKind::Keyword,
                            background: None,
                        });
                        grapheme_cursor.set_index(end);

                        continue;
                    }

                    default_end = end;

                    if syntax.has_identifiers {
                        default_foreground = HighlightKind::Identifier;
                    }
                }

                for (i, range) in syntax.ranges.iter().enumerate() {
                    let HighlightResult::Range {
                        start,
                        end,
                        is_finished,
                    } = Self::match_range(line, grapheme_cursor.index(), range, false)
                    else {
                        continue;
                    };

                    if start > grapheme_cursor.index() {
                        self.highlight_line(&line[..start], syntax, grapheme_cursor, y);
                    }

                    self.highlighted_lines[y].push(Highlight {
                        start,
                        end,
                        foreground: range.kind,
                        background: None,
                    });
                    grapheme_cursor.set_index(end);

                    if !is_finished {
                        self.highlighted_lines[y].unfinished_range_index = Some(i);
                    }

                    continue 'tokenize;
                }

                for token in &syntax.tokens {
                    let HighlightResult::Token { start, end } =
                        Self::match_token(line, grapheme_cursor.index(), token)
                    else {
                        continue;
                    };

                    if start > grapheme_cursor.index() {
                        self.highlight_line(&line[..start], syntax, grapheme_cursor, y);
                    }

                    self.highlighted_lines[y].push(Highlight {
                        start,
                        end,
                        foreground: token.kind,
                        background: None,
                    });
                    grapheme_cursor.set_index(end);

                    continue 'tokenize;
                }
            }

            self.highlighted_lines[y].push(Highlight {
                start: default_start,
                end: default_end,
                foreground: default_foreground,
                background: None,
            });
            grapheme_cursor.set_index(default_end);
        }
    }

    pub fn highlight_line_from_terminal_colors(
        &mut self,
        colors: &[(TerminalHighlightKind, TerminalHighlightKind)],
        y: usize,
    ) {
        if self.highlighted_lines.len() <= y {
            self.highlighted_lines.resize(y + 1, HighlightedLine::new());
        }

        let highlighted_line = &mut self.highlighted_lines[y];

        highlighted_line.clear();

        for (x, (foreground, background)) in colors.iter().enumerate() {
            highlighted_line.push(Highlight {
                start: x,
                end: x + 1,
                foreground: HighlightKind::Terminal(*foreground),
                background: Some(HighlightKind::Terminal(*background)),
            });
        }
    }

    pub fn scroll_highlighted_lines(&mut self, region: RangeInclusive<usize>, delta_y: isize) {
        let start = *region.start();
        let end = *region.end();
        let end = end.min(self.highlighted_lines.len().saturating_sub(1));

        let (start, end) = if delta_y < 0 {
            (end, start)
        } else {
            (start, end)
        };

        for _ in 0..delta_y.abs() {
            let mut highlighted_line = self.highlighted_lines.remove(start);
            highlighted_line.clear();
            self.highlighted_lines.insert(end, highlighted_line);
        }
    }

    pub fn highlighted_lines(&self) -> &[HighlightedLine] {
        &self.highlighted_lines
    }
}
