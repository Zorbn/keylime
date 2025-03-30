use serde::Deserialize;

use crate::ui::color::Color;

use super::{
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

#[derive(Clone, Copy)]
pub struct Highlight {
    pub start: isize,
    pub end: isize,
    pub foreground: HighlightKind,
    pub background: Option<HighlightKind>,
}

pub enum HighlightResult {
    Token {
        start: isize,
        end: isize,
    },
    Range {
        start: isize,
        end: isize,
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

    pub fn match_identifier(line: &Line, start: isize) -> HighlightResult {
        HighlightResult::None
        // let mut i = start;

        // if line[i] != '_' && !line[i].is_alphabetic() {
        //     return HighlightResult::None;
        // }

        // i += 1;

        // while i < line.len() {
        //     let c = line[i];

        //     if c != '_' && !c.is_alphanumeric() {
        //         break;
        //     }

        //     i += 1;
        // }

        // HighlightResult::Token { start, end: i }
    }

    fn match_token(line: &Line, start: isize, token: &SyntaxToken) -> HighlightResult {
        HighlightResult::None
        // match token.pattern.match_text(line, start) {
        //     Some(pattern_match) => HighlightResult::Token {
        //         start: pattern_match.start,
        //         end: pattern_match.end,
        //     },
        //     None => HighlightResult::None,
        // }
    }

    fn match_range(
        line: &Line,
        start: isize,
        range: &SyntaxRange,
        is_in_progress: bool,
    ) -> HighlightResult {
        HighlightResult::None
        // let mut i = start;

        // if !is_in_progress {
        //     match range.start.match_text(line, start) {
        //         Some(pattern_match) => i = pattern_match.end,
        //         None => return HighlightResult::None,
        //     }
        // }

        // while i < line.len() {
        //     if Some(line[i]) == range.escape {
        //         i += 2;
        //         continue;
        //     }

        //     if let Some(pattern_match) = range.end.match_text(line, i) {
        //         return HighlightResult::Range {
        //             start,
        //             end: pattern_match.end,
        //             is_finished: true,
        //         };
        //     }

        //     i += 1;
        // }

        // HighlightResult::Range {
        //     start,
        //     end: i.min(line.len()),
        //     is_finished: false,
        // }
    }

    pub fn update(&mut self, lines: &[Line], syntax: &Syntax, start_y: usize, end_y: usize) {
        if self.highlighted_lines.len() < lines.len() {
            self.highlighted_lines
                .resize(lines.len(), HighlightedLine::new());
        }

        for (y, line) in lines.iter().enumerate().take(end_y + 1).skip(start_y) {
            self.highlighted_lines[y].clear();

            let mut x = 0;

            if y > 0 {
                let last_highlighted_line = &self.highlighted_lines[y - 1];

                if let Some(unfinished_range_index) = last_highlighted_line.unfinished_range_index {
                    let range = &syntax.ranges[unfinished_range_index];

                    if let HighlightResult::Range {
                        start,
                        end,
                        is_finished,
                    } = Self::match_range(line, x, range, true)
                    {
                        self.highlighted_lines[y].push(Highlight {
                            start,
                            end,
                            foreground: range.kind,
                            background: None,
                        });
                        x = end;

                        if !is_finished {
                            self.highlighted_lines[y].unfinished_range_index =
                                Some(unfinished_range_index);
                        }
                    }
                }
            }

            self.highlight_line(line, syntax, x, y);
        }
    }

    fn highlight_line(&mut self, line: &Line, syntax: &Syntax, mut x: isize, y: usize) {
        // 'tokenize: while x < line.len() {
        //     let mut default_end = x + 1;

        //     if !line[x].is_whitespace() {
        //         if let HighlightResult::Token { start, end } = Self::match_identifier(line, x) {
        //             let identifier = &line[start..end];

        //             if syntax.keywords.contains(identifier) {
        //                 self.highlighted_lines[y].push(Highlight {
        //                     start,
        //                     end,
        //                     foreground: HighlightKind::Keyword,
        //                     background: None,
        //                 });
        //                 x = end;

        //                 continue;
        //             }

        //             default_end = end;
        //         }

        //         for (i, range) in syntax.ranges.iter().enumerate() {
        //             let HighlightResult::Range {
        //                 start,
        //                 end,
        //                 is_finished,
        //             } = Self::match_range(line, x, range, false)
        //             else {
        //                 continue;
        //             };

        //             if start > x {
        //                 self.highlight_line(&line[..start], syntax, x, y);
        //             }

        //             self.highlighted_lines[y].push(Highlight {
        //                 start,
        //                 end,
        //                 foreground: range.kind,
        //                 background: None,
        //             });
        //             x = end;

        //             if !is_finished {
        //                 self.highlighted_lines[y].unfinished_range_index = Some(i);
        //             }

        //             continue 'tokenize;
        //         }

        //         for token in &syntax.tokens {
        //             let HighlightResult::Token { start, end } = Self::match_token(line, x, token)
        //             else {
        //                 continue;
        //             };

        //             if start > x {
        //                 self.highlight_line(&line[..start], syntax, x, y);
        //             }

        //             self.highlighted_lines[y].push(Highlight {
        //                 start,
        //                 end,
        //                 foreground: token.kind,
        //                 background: None,
        //             });
        //             x = end;

        //             continue 'tokenize;
        //         }
        //     }

        //     self.highlighted_lines[y].push(Highlight {
        //         start: x,
        //         end: default_end,
        //         foreground: HighlightKind::Normal,
        //         background: None,
        //     });
        //     x = default_end;
        // }
    }

    pub fn highlight_line_from_terminal_colors(
        &mut self,
        colors: &[(TerminalHighlightKind, TerminalHighlightKind)],
        y: usize,
    ) {
        // if self.highlighted_lines.len() <= y {
        //     self.highlighted_lines.resize(y + 1, HighlightedLine::new());
        // }

        // let highlighted_line = &mut self.highlighted_lines[y];

        // highlighted_line.clear();

        // for (x, (foreground, background)) in colors.iter().enumerate() {
        //     highlighted_line.push(Highlight {
        //         start: x,
        //         end: x + 1,
        //         foreground: HighlightKind::Terminal(*foreground),
        //         background: Some(HighlightKind::Terminal(*background)),
        //     });
        // }
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
