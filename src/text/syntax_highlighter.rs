use serde::Deserialize;

use crate::ui::color::Color;

use super::{
    grapheme::{self, GraphemeSelection, GraphemeSelector},
    line::Line,
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
    pub start: GraphemeSelection,
    pub end: GraphemeSelection,
    pub foreground: HighlightKind,
    pub background: Option<HighlightKind>,
}

pub enum HighlightResult {
    Token {
        start: GraphemeSelection,
        end: GraphemeSelection,
    },
    Range {
        start: GraphemeSelection,
        end: GraphemeSelection,
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
            if last_highlight.end.index() == highlight.start.index()
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

    pub fn match_identifier(text: &str, start: GraphemeSelection) -> HighlightResult {
        let start_grapheme = start.grapheme(text);

        if start_grapheme != "_" && !grapheme::is_alphabetic(start_grapheme) {
            return HighlightResult::None;
        }

        let mut grapheme_selector = GraphemeSelector::with_selection(start, text);
        grapheme_selector.next_boundary(text);

        while !grapheme_selector.is_at_end(text) {
            let grapheme = grapheme_selector.grapheme(text);

            if grapheme != "_" && !grapheme::is_alphanumeric(grapheme) {
                break;
            }

            grapheme_selector.next_boundary(text);
        }

        HighlightResult::Token {
            start,
            end: grapheme_selector.selection(),
        }
    }

    fn match_token(text: &str, start: GraphemeSelection, token: &SyntaxToken) -> HighlightResult {
        match token.pattern.match_text(text, start) {
            Some(pattern_match) => HighlightResult::Token {
                start: pattern_match.start,
                end: pattern_match.end,
            },
            None => HighlightResult::None,
        }
    }

    fn match_range(
        text: &str,
        start: GraphemeSelection,
        range: &SyntaxRange,
        is_in_progress: bool,
    ) -> HighlightResult {
        let mut grapheme_selector = GraphemeSelector::with_selection(start, text);

        if !is_in_progress {
            match range.start.match_text(text, start) {
                Some(pattern_match) => grapheme_selector.set_selection(pattern_match.end),
                None => return HighlightResult::None,
            }
        }

        while !grapheme_selector.is_at_end(text) {
            // TODO:
            // if Some(grapheme_selector.grapheme(text)) == range.escape {
            //     grapheme_selector.next_boundary(text);
            //     grapheme_selector.next_boundary(text);
            //     continue;
            // }

            if let Some(pattern_match) = range.end.match_text(text, grapheme_selector.selection()) {
                return HighlightResult::Range {
                    start,
                    end: pattern_match.end,
                    is_finished: true,
                };
            }

            grapheme_selector.next_boundary(text);
        }

        HighlightResult::Range {
            start,
            end: grapheme_selector.selection(),
            is_finished: false,
        }
    }

    pub fn update(&mut self, lines: &[Line], syntax: &Syntax, start_y: usize, end_y: usize) {
        if self.highlighted_lines.len() < lines.len() {
            self.highlighted_lines
                .resize(lines.len(), HighlightedLine::new());
        }

        for (y, line) in lines.iter().enumerate().take(end_y + 1).skip(start_y) {
            self.highlighted_lines[y].clear();

            let text = &line[..];
            let mut grapheme_selector = GraphemeSelector::new(text);

            if y > 0 {
                let last_highlighted_line = &self.highlighted_lines[y - 1];

                if let Some(unfinished_range_index) = last_highlighted_line.unfinished_range_index {
                    let range = &syntax.ranges[unfinished_range_index];

                    if let HighlightResult::Range {
                        start,
                        end,
                        is_finished,
                    } = Self::match_range(text, grapheme_selector.selection(), range, true)
                    {
                        self.highlighted_lines[y].push(Highlight {
                            start,
                            end,
                            foreground: range.kind,
                            background: None,
                        });
                        grapheme_selector.set_selection(end);

                        if !is_finished {
                            self.highlighted_lines[y].unfinished_range_index =
                                Some(unfinished_range_index);
                        }
                    }
                }
            }

            self.highlight_line(text, syntax, &mut grapheme_selector, y);
        }
    }

    fn highlight_line(
        &mut self,
        text: &str,
        syntax: &Syntax,
        grapheme_selector: &mut GraphemeSelector,
        y: usize,
    ) {
        'tokenize: while !grapheme_selector.is_at_end(text) {
            let default_start = grapheme_selector.selection();
            grapheme_selector.next_boundary(text);

            let mut default_end = grapheme_selector.selection();
            grapheme_selector.set_selection(default_start);

            if !grapheme::is_whitespace(grapheme_selector.grapheme(text)) {
                if let HighlightResult::Token { start, end } =
                    Self::match_identifier(text, grapheme_selector.selection())
                {
                    let identifier = GraphemeSelection::grapheme_range(&start, &end, text);

                    if syntax.keywords.contains(identifier) {
                        self.highlighted_lines[y].push(Highlight {
                            start,
                            end,
                            foreground: HighlightKind::Keyword,
                            background: None,
                        });
                        grapheme_selector.set_selection(end);

                        continue;
                    }

                    default_end = end;
                }

                for (i, range) in syntax.ranges.iter().enumerate() {
                    let HighlightResult::Range {
                        start,
                        end,
                        is_finished,
                    } = Self::match_range(text, grapheme_selector.selection(), range, false)
                    else {
                        continue;
                    };

                    if start.index() > grapheme_selector.index() {
                        self.highlight_line(start.range_before(text), syntax, grapheme_selector, y);
                    }

                    self.highlighted_lines[y].push(Highlight {
                        start,
                        end,
                        foreground: range.kind,
                        background: None,
                    });
                    grapheme_selector.set_selection(end);

                    if !is_finished {
                        self.highlighted_lines[y].unfinished_range_index = Some(i);
                    }

                    continue 'tokenize;
                }

                for token in &syntax.tokens {
                    let HighlightResult::Token { start, end } =
                        Self::match_token(text, grapheme_selector.selection(), token)
                    else {
                        continue;
                    };

                    if start.index() > grapheme_selector.index() {
                        self.highlight_line(start.range_before(text), syntax, grapheme_selector, y);
                    }

                    self.highlighted_lines[y].push(Highlight {
                        start,
                        end,
                        foreground: token.kind,
                        background: None,
                    });
                    grapheme_selector.set_selection(end);

                    continue 'tokenize;
                }
            }

            self.highlighted_lines[y].push(Highlight {
                start: default_start,
                end: default_end,
                foreground: HighlightKind::Normal,
                background: None,
            });
            grapheme_selector.set_selection(default_end);
        }
    }

    pub fn highlight_line_from_terminal_colors(
        &mut self,
        lines: &[Line],
        colors: &[(TerminalHighlightKind, TerminalHighlightKind)],
        y: usize,
    ) {
        if self.highlighted_lines.len() <= y {
            self.highlighted_lines.resize(y + 1, HighlightedLine::new());
        }

        let highlighted_line = &mut self.highlighted_lines[y];

        highlighted_line.clear();

        let text = &lines[y][..];
        let mut grapheme_selector = GraphemeSelector::new(text);

        for (foreground, background) in colors {
            let start = grapheme_selector.selection();
            grapheme_selector.next_boundary(text);

            highlighted_line.push(Highlight {
                start,
                end: grapheme_selector.selection(),
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
