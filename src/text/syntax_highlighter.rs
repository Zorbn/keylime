use serde::Deserialize;
use unicode_segmentation::GraphemeCursor;

use crate::ui::color::Color;

use super::{
    grapheme,
    syntax::{Syntax, SyntaxRange, SyntaxToken},
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
    Comment,
    Keyword,
    Function,
    Number,
    Symbol,
    String,
    Meta,
    Terminal(TerminalHighlightKind),
}

#[derive(Clone)]
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

    pub fn match_identifier(line: &str, start: usize) -> HighlightResult {
        let mut grapheme_cursor = GraphemeCursor::new(start, line.len(), true);
        let start_grapheme = grapheme::at(grapheme_cursor.cur_cursor(), line);

        if start_grapheme != "_" && !grapheme::is_alphabetic(start_grapheme) {
            return HighlightResult::None;
        }

        grapheme_cursor.next_boundary(line, 0);

        while grapheme_cursor.cur_cursor() < line.len() {
            let grapheme = grapheme::at(grapheme_cursor.cur_cursor(), line);

            if grapheme != "_" && !grapheme::is_alphanumeric(grapheme) {
                break;
            }

            grapheme_cursor.next_boundary(line, 0);
        }

        HighlightResult::Token {
            start,
            end: grapheme_cursor.cur_cursor(),
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
        let mut grapheme_cursor = GraphemeCursor::new(start, line.len(), true);

        if !is_in_progress {
            match range.start.match_text(line, start) {
                Some(pattern_match) => grapheme_cursor.set_cursor(pattern_match.end),
                None => return HighlightResult::None,
            }
        }

        while grapheme_cursor.cur_cursor() < line.len() {
            // TODO:
            // if Some(grapheme_selector.grapheme(text)) == range.escape {
            //     grapheme_selector.next_boundary(text);
            //     grapheme_selector.next_boundary(text);
            //     continue;
            // }

            if let Some(pattern_match) = range.end.match_text(line, grapheme_cursor.cur_cursor()) {
                return HighlightResult::Range {
                    start,
                    end: pattern_match.end,
                    is_finished: true,
                };
            }

            grapheme_cursor.next_boundary(line, 0);
        }

        HighlightResult::Range {
            start,
            end: grapheme_cursor.cur_cursor(),
            is_finished: false,
        }
    }

    pub fn update(&mut self, lines: &[String], syntax: &Syntax, start_y: usize, end_y: usize) {
        if self.highlighted_lines.len() < lines.len() {
            self.highlighted_lines
                .resize(lines.len(), HighlightedLine::new());
        }

        for (y, line) in lines.iter().enumerate().take(end_y + 1).skip(start_y) {
            self.highlighted_lines[y].clear();

            let mut grapheme_cursor = GraphemeCursor::new(0, line.len(), true);

            if y > 0 {
                let last_highlighted_line = &self.highlighted_lines[y - 1];

                if let Some(unfinished_range_index) = last_highlighted_line.unfinished_range_index {
                    let range = &syntax.ranges[unfinished_range_index];

                    if let HighlightResult::Range {
                        start,
                        end,
                        is_finished,
                    } = Self::match_range(line, grapheme_cursor.cur_cursor(), range, true)
                    {
                        self.highlighted_lines[y].push(Highlight {
                            start,
                            end,
                            foreground: range.kind,
                            background: None,
                        });
                        grapheme_cursor.set_cursor(end);

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
        'tokenize: while grapheme_cursor.cur_cursor() < line.len() {
            let default_start = grapheme_cursor.cur_cursor();
            grapheme_cursor.next_boundary(line, 0);

            let mut default_end = grapheme_cursor.cur_cursor();
            grapheme_cursor.set_cursor(default_start);

            if !grapheme::is_whitespace(grapheme::at(grapheme_cursor.cur_cursor(), line)) {
                if let HighlightResult::Token { start, end } =
                    Self::match_identifier(line, grapheme_cursor.cur_cursor())
                {
                    let identifier = &line[start..end];

                    if syntax.keywords.contains(identifier) {
                        self.highlighted_lines[y].push(Highlight {
                            start,
                            end,
                            foreground: HighlightKind::Keyword,
                            background: None,
                        });
                        grapheme_cursor.set_cursor(end);

                        continue;
                    }

                    default_end = end;
                }

                for (i, range) in syntax.ranges.iter().enumerate() {
                    let HighlightResult::Range {
                        start,
                        end,
                        is_finished,
                    } = Self::match_range(line, grapheme_cursor.cur_cursor(), range, false)
                    else {
                        continue;
                    };

                    if start > grapheme_cursor.cur_cursor() {
                        self.highlight_line(&line[..start], syntax, grapheme_cursor, y);
                    }

                    self.highlighted_lines[y].push(Highlight {
                        start,
                        end,
                        foreground: range.kind,
                        background: None,
                    });
                    grapheme_cursor.set_cursor(end);

                    if !is_finished {
                        self.highlighted_lines[y].unfinished_range_index = Some(i);
                    }

                    continue 'tokenize;
                }

                for token in &syntax.tokens {
                    let HighlightResult::Token { start, end } =
                        Self::match_token(line, grapheme_cursor.cur_cursor(), token)
                    else {
                        continue;
                    };

                    if start > grapheme_cursor.cur_cursor() {
                        self.highlight_line(&line[..start], syntax, grapheme_cursor, y);
                    }

                    self.highlighted_lines[y].push(Highlight {
                        start,
                        end,
                        foreground: token.kind,
                        background: None,
                    });
                    grapheme_cursor.set_cursor(end);

                    continue 'tokenize;
                }
            }

            self.highlighted_lines[y].push(Highlight {
                start: default_start,
                end: default_end,
                foreground: HighlightKind::Normal,
                background: None,
            });
            grapheme_cursor.set_cursor(default_end);
        }
    }

    pub fn highlight_line_from_terminal_colors(
        &mut self,
        lines: &[String],
        colors: &[(TerminalHighlightKind, TerminalHighlightKind)],
        y: usize,
    ) {
        if self.highlighted_lines.len() <= y {
            self.highlighted_lines.resize(y + 1, HighlightedLine::new());
        }

        let highlighted_line = &mut self.highlighted_lines[y];

        highlighted_line.clear();

        let text = &lines[y][..];
        let mut grapheme_cursor = GraphemeCursor::new(0, text.len(), true);

        for (foreground, background) in colors {
            let start = grapheme_cursor.cur_cursor();
            grapheme_cursor.next_boundary(text, 0);

            highlighted_line.push(Highlight {
                start,
                end: grapheme_cursor.cur_cursor(),
                foreground: HighlightKind::Terminal(*foreground),
                background: Some(HighlightKind::Terminal(*background)),
            });
        }
    }

    pub fn recycle_highlighted_lines_up_to_y(&mut self, y: usize) {
        for _ in 0..y {
            let highlighted_line = self.highlighted_lines.remove(0);
            self.highlighted_lines.push(highlighted_line);
        }
    }

    pub fn highlighted_lines(&self) -> &[HighlightedLine] {
        &self.highlighted_lines
    }
}
