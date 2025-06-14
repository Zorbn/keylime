mod lsp;

#[cfg(test)]
mod tests;

use std::{
    fmt::Display,
    fs::{read_to_string, File},
    io::{self, Write},
    mem::take,
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use lsp::DocLspState;

use crate::{
    bit_field::define_bit_field,
    ctx::{ctx_with_time, Ctx},
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    lsp::types::DecodedDiagnostic,
    normalizable::Normalizable,
    platform::gfx::Gfx,
    pool::{Pooled, STRING_POOL},
    text::grapheme,
};

use super::{
    action_history::{Action, ActionHistory, ActionKind},
    cursor::Cursor,
    cursor_index::{CursorIndex, CursorIndices},
    grapheme::{CharCursor, CharIterator, GraphemeCursor, GraphemeIterator},
    grapheme_category::GraphemeCategory,
    selection::Selection,
    syntax::Syntax,
    syntax_highlighter::{HighlightedLine, SyntaxHighlighter, TerminalHighlightKind},
    tokenizer::Tokenizer,
    trie::Trie,
};

macro_rules! action_history {
    ($self:ident, $action_kind:expr) => {
        match $action_kind {
            ActionKind::Done | ActionKind::Redone => &mut $self.undo_history,
            ActionKind::Undone => &mut $self.redo_history,
        }
    };
}

#[derive(Debug, Clone, Copy)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl Default for LineEnding {
    fn default() -> Self {
        if cfg!(target_os = "windows") {
            Self::CrLf
        } else {
            Self::Lf
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DocFlag {
    TrackHistory,
    TrackSaving,
    UpdateCursors,
    AllowLanguageServer,
    AllowMultipleLines,
    AllowScrollingPastBottom,
    RecenterOnBottom,
    MatchPairs,
    ShowGutter,
}

define_bit_field!(DocFlags, DocFlag, u16);

impl DocFlags {
    pub const RAW: Self = Self::from(DocFlag::AllowMultipleLines);

    pub const SINGLE_LINE: Self = Self::NONE
        .with(DocFlag::TrackHistory)
        .with(DocFlag::TrackSaving)
        .with(DocFlag::UpdateCursors);

    pub const MULTI_LINE: Self = Self::SINGLE_LINE
        .with(DocFlag::AllowLanguageServer)
        .with(DocFlag::AllowMultipleLines)
        .with(DocFlag::AllowScrollingPastBottom)
        .with(DocFlag::MatchPairs)
        .with(DocFlag::ShowGutter);

    pub const TERMINAL: Self = Self::RAW.with(DocFlag::RecenterOnBottom);
}

#[derive(Debug, Default)]
pub enum DocPath {
    #[default]
    None,
    InMemory(Pooled<PathBuf>),
    OnDrive(Pooled<PathBuf>),
}

impl DocPath {
    pub fn some(&self) -> Option<&Pooled<PathBuf>> {
        match &self {
            Self::None => None,
            Self::InMemory(path) => Some(path),
            Self::OnDrive(path) => Some(path),
        }
    }

    pub fn some_path(&self) -> Option<&Path> {
        self.some().map(AsRef::as_ref)
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn on_drive(&self) -> Option<&Path> {
        match &self {
            Self::OnDrive(path) => Some(path),
            _ => None,
        }
    }
}

// One change for File::create, and one change for writing.
#[cfg(target_os = "windows")]
const EXPECTED_CHANGE_COUNT_ON_SAVE: usize = 2;

#[cfg(target_os = "macos")]
const EXPECTED_CHANGE_COUNT_ON_SAVE: usize = 1;

pub struct Doc {
    display_name: Option<Pooled<String>>,
    path: DocPath,
    is_saved: bool,
    expected_change_count: usize,
    version: usize,
    usages: usize,
    do_skip_shifting: bool,

    lines: Vec<Pooled<String>>,
    cursors: Vec<Cursor>,
    line_ending: LineEnding,

    undo_history: ActionHistory,
    redo_history: ActionHistory,

    syntax_highlighter: SyntaxHighlighter,
    unhighlighted_line_y: usize,
    tokenizer: Tokenizer,
    needs_tokenization: bool,

    lsp_state: DocLspState,

    flags: DocFlags,
}

impl Doc {
    pub fn new(
        path: Option<Pooled<PathBuf>>,
        display_name: Option<Pooled<String>>,
        flags: DocFlags,
    ) -> Self {
        assert!(path.as_ref().is_none_or(Normalizable::is_normal));

        let lines = vec![STRING_POOL.new_item()];

        let (path, is_saved) = match path {
            Some(path) => (DocPath::InMemory(path), false),
            _ => (DocPath::None, true),
        };

        let mut doc = Self {
            display_name,
            path,
            is_saved,
            expected_change_count: 0,
            version: 0,
            usages: 0,
            do_skip_shifting: false,

            lines,
            cursors: Vec::new(),
            line_ending: LineEnding::default(),

            undo_history: ActionHistory::new(),
            redo_history: ActionHistory::new(),

            syntax_highlighter: SyntaxHighlighter::new(),
            unhighlighted_line_y: 0,
            tokenizer: Tokenizer::new(),
            needs_tokenization: false,

            lsp_state: Default::default(),

            flags,
        };

        doc.reset_cursors();

        doc
    }

    pub fn add_usage(&mut self) {
        self.usages += 1;
    }

    pub fn remove_usage(&mut self) {
        self.usages -= 1;
    }

    pub fn usages(&self) -> usize {
        self.usages
    }

    pub fn move_position_to_next_grapheme(&self, position: &mut Position) -> bool {
        let Some(line) = self.get_line(position.y) else {
            return false;
        };

        let mut grapheme_cursor = GraphemeCursor::new(position.x, line.len());

        match grapheme_cursor.next_boundary(line) {
            Some(new_x) => {
                position.x = new_x;
                true
            }
            _ => false,
        }
    }

    pub fn move_position_to_previous_grapheme(&self, position: &mut Position) -> bool {
        let Some(line) = self.get_line(position.y) else {
            return false;
        };

        let mut grapheme_cursor = GraphemeCursor::new(position.x, line.len());

        match grapheme_cursor.previous_boundary(line) {
            Some(new_x) => {
                position.x = new_x;
                true
            }
            _ => false,
        }
    }

    pub fn move_position(
        &self,
        position: Position,
        delta_x: isize,
        delta_y: isize,
        gfx: &mut Gfx,
    ) -> Position {
        self.move_position_with_desired_visual_x(position, delta_x, delta_y, None, gfx)
    }

    pub fn move_position_with_desired_visual_x(
        &self,
        position: Position,
        delta_x: isize,
        delta_y: isize,
        desired_visual_x: Option<usize>,
        gfx: &mut Gfx,
    ) -> Position {
        let mut position = self.clamp_position(position);

        if let Some(new_y) = position.y.checked_add_signed(delta_y) {
            position.y = new_y;
        } else {
            return Position::new(0, 0);
        };

        if position.y >= self.lines.len() {
            return Position::new(self.lines.last().unwrap().len(), self.lines.len() - 1);
        }

        if delta_y != 0 {
            if let Some(desired_visual_x) = desired_visual_x {
                position.x = gfx.find_x_for_visual_x(&self.lines[position.y][..], desired_visual_x);
            } else if position.x > self.line_len(position.y) {
                position.x = self.line_len(position.y);
            }
        }

        if delta_x < 0 {
            for _ in 0..delta_x.abs() {
                if self.move_position_to_previous_grapheme(&mut position) {
                    continue;
                }

                if position.y == 0 {
                    break;
                }

                position.y -= 1;
                position.x = self.lines[position.y].len();
            }
        } else {
            for _ in 0..delta_x {
                if self.move_position_to_next_grapheme(&mut position) {
                    continue;
                }

                if position.y == self.lines.len() - 1 {
                    break;
                }

                position.x = 0;
                position.y += 1;
            }
        }

        position
    }

    pub fn move_position_skipping_category(
        &self,
        position: Position,
        delta_x: isize,
        category: GraphemeCategory,
        gfx: &mut Gfx,
    ) -> Position {
        let mut position = self.clamp_position(position);
        let side_offset = Self::side_offset(delta_x);

        loop {
            let current_category = GraphemeCategory::new(self.grapheme(self.move_position(
                position,
                side_offset,
                0,
                gfx,
            )));

            let next_position = self.move_position(position, delta_x, 0, gfx);

            if current_category != category
                || current_category == GraphemeCategory::Newline
                || next_position == position
            {
                break;
            }

            position = next_position;
        }

        position
    }

    pub fn move_position_to_next_word(
        &self,
        position: Position,
        delta_x: isize,
        gfx: &mut Gfx,
    ) -> Position {
        let starting_position =
            self.move_position_skipping_category(position, delta_x, GraphemeCategory::Space, gfx);

        let side_offset = Self::side_offset(delta_x);
        let starting_category = GraphemeCategory::new(self.grapheme(self.move_position(
            starting_position,
            side_offset,
            0,
            gfx,
        )));

        let ending_position = self.move_position_skipping_category(
            starting_position,
            delta_x,
            starting_category,
            gfx,
        );

        if ending_position == position {
            self.move_position(ending_position, delta_x, 0, gfx)
        } else {
            ending_position
        }
    }

    pub fn move_position_skipping_lines(
        &self,
        position: Position,
        delta_y: isize,
        do_skip_empty_lines: bool,
        gfx: &mut Gfx,
    ) -> Position {
        let mut position = self.clamp_position(position);

        loop {
            let current_line_is_empty = self.line_len(position.y) == 0;
            let next_position = self.move_position(position, 0, delta_y, gfx);

            if current_line_is_empty != do_skip_empty_lines || next_position == position {
                break;
            }

            position = next_position;
        }

        position
    }

    pub fn move_position_to_next_paragraph(
        &self,
        position: Position,
        delta_y: isize,
        do_skip_leading_whitespace: bool,
        gfx: &mut Gfx,
    ) -> Position {
        let starting_position = if do_skip_leading_whitespace {
            self.move_position_skipping_lines(position, delta_y, true, gfx)
        } else {
            self.clamp_position(position)
        };

        let starting_line_is_empty = self.line_len(starting_position.y) == 0;

        self.move_position_skipping_lines(starting_position, delta_y, starting_line_is_empty, gfx)
    }

    fn side_offset(direction_x: isize) -> isize {
        if direction_x < 0 {
            -1
        } else {
            0
        }
    }

    pub fn get_line(&self, y: usize) -> Option<&str> {
        self.lines.get(y).map(|line| line.as_str())
    }

    pub fn line_len(&self, y: usize) -> usize {
        if let Some(line) = self.get_line(y) {
            line.len()
        } else {
            0
        }
    }

    pub fn line_end(&self, y: usize) -> Position {
        Position::new(self.line_len(y), y)
    }

    pub fn line_start(&self, y: usize) -> usize {
        let Some(line) = self.get_line(y) else {
            return 0;
        };

        let mut start = 0;

        for grapheme in GraphemeIterator::new(line) {
            if !grapheme::is_whitespace(grapheme) {
                break;
            }

            start += grapheme.len();
        }

        start
    }

    pub fn is_line_whitespace(&self, y: usize) -> bool {
        self.line_start(y) == self.line_len(y)
    }

    pub fn comment_line(&mut self, comment: &str, position: Position, ctx: &mut Ctx) {
        self.insert(position, " ", ctx);
        self.insert(position, comment, ctx);
    }

    pub fn uncomment_line(&mut self, comment: &str, y: usize, ctx: &mut Ctx) -> bool {
        if !self.is_line_commented(comment, y) {
            return false;
        }

        let start = Position::new(self.line_start(y), y);
        let end = Position::new(start.x + comment.len(), y);

        self.delete(start, end, ctx);

        if self.grapheme(start) == " " {
            let end = self.move_position(start, 1, 0, ctx.gfx);

            self.delete(start, end, ctx);
        }

        true
    }

    pub fn is_line_commented(&self, comment: &str, y: usize) -> bool {
        let Some(line) = self.get_line(y) else {
            return false;
        };

        let start = self.line_start(y);

        if start + comment.len() >= line.len() {
            return false;
        }

        let mut grapheme_cursor = GraphemeCursor::new(start, comment.len());

        for comment_grapheme in GraphemeIterator::new(comment) {
            let start = grapheme_cursor.index();
            let Some(end) = grapheme_cursor.next_boundary(line) else {
                break;
            };

            let line_grapheme = &line[start..end];

            if comment_grapheme != line_grapheme {
                return false;
            }
        }

        true
    }

    pub fn toggle_comments_at_cursors(&mut self, ctx: &mut Ctx) {
        let Some(language) = ctx.config.get_language_for_doc(self) else {
            return;
        };

        let comment = &language.comment;

        for index in self.cursor_indices() {
            let cursor = self.cursor(index);

            let selection = cursor
                .get_selection()
                .unwrap_or(Selection {
                    start: cursor.position,
                    end: cursor.position,
                })
                .trim();

            let mut min_comment_x = usize::MAX;
            let mut did_uncomment = false;

            for y in selection.start.y..=selection.end.y {
                if self.is_line_whitespace(y) {
                    continue;
                }

                min_comment_x = min_comment_x.min(self.line_start(y));
                did_uncomment = self.uncomment_line(comment, y, ctx) || did_uncomment;
            }

            if did_uncomment {
                continue;
            }

            for y in selection.start.y..=selection.end.y {
                if self.is_line_whitespace(y) {
                    continue;
                }

                self.comment_line(comment, Position::new(min_comment_x, y), ctx);
            }
        }
    }

    pub fn indent_lines_at_cursor(&mut self, index: CursorIndex, do_unindent: bool, ctx: &mut Ctx) {
        let cursor = self.cursor(index);

        let selection = cursor.get_selection();
        let has_selection = selection.is_some();

        let selection = selection
            .unwrap_or(Selection {
                start: cursor.position,
                end: cursor.position,
            })
            .trim();

        for y in selection.start.y..=selection.end.y {
            if has_selection && self.line_len(y) == 0 {
                continue;
            }

            if do_unindent {
                self.unindent_line(y, ctx);
            } else {
                self.indent_line(y, ctx);
            }
        }
    }

    pub fn indent_lines_at_cursors(&mut self, do_unindent: bool, ctx: &mut Ctx) {
        for index in self.cursor_indices() {
            self.indent_lines_at_cursor(index, do_unindent, ctx);
        }
    }

    fn indent_line(&mut self, y: usize, ctx: &mut Ctx) {
        self.indent(Position::new(0, y), ctx);
    }

    fn unindent_line(&mut self, y: usize, ctx: &mut Ctx) {
        let end = Position::new(self.line_start(y), y);

        self.unindent(end, ctx);
    }

    fn indent(&mut self, mut start: Position, ctx: &mut Ctx) {
        let indent_width = ctx.config.indent_width_for_doc(self);
        let grapheme = indent_width.grapheme();

        for _ in 0..indent_width.grapheme_count() {
            self.insert(start, grapheme, ctx);
            start = self.move_position(start, 1, 0, ctx.gfx);
        }
    }

    fn unindent(&mut self, end: Position, ctx: &mut Ctx) {
        let start = self.indent_start(end, ctx);
        self.delete(start, end, ctx);
    }

    pub fn indent_start(&self, end: Position, ctx: &mut Ctx) -> Position {
        let indent_width = ctx.config.indent_width_for_doc(self);

        let mut start = self.move_position(end, -1, 0, ctx.gfx);
        let start_grapheme = self.grapheme(start);

        match start_grapheme {
            " " => {
                let indent_width = (end.x - 1) % indent_width.grapheme_count() + 1;

                for _ in 1..indent_width {
                    let next_start = self.move_position(start, -1, 0, ctx.gfx);

                    if self.grapheme(next_start) != " " {
                        break;
                    }

                    start = next_start;
                }

                start
            }
            "\t" => start,
            _ => end,
        }
    }

    pub fn indent_at_cursor(&mut self, index: CursorIndex, ctx: &mut Ctx) {
        let start = self.cursor(index).position;
        self.indent(start, ctx);
    }

    pub fn indent_at_cursors(&mut self, ctx: &mut Ctx) {
        for index in self.cursor_indices() {
            self.indent_at_cursor(index, ctx);
        }
    }

    pub fn move_cursor(
        &mut self,
        index: CursorIndex,
        delta_x: isize,
        delta_y: isize,
        should_select: bool,
        gfx: &mut Gfx,
    ) {
        self.update_cursor_selection(index, should_select);

        let cursor = self.cursor(index);
        let start_position = cursor.position;
        let desired_visual_x = cursor.desired_visual_x;

        self.cursor_mut(index).position = self.move_position_with_desired_visual_x(
            start_position,
            delta_x,
            delta_y,
            Some(desired_visual_x),
            gfx,
        );

        if delta_x != 0 {
            self.update_cursor_desired_visual_x(index, gfx);
        }
    }

    pub fn move_cursors(
        &mut self,
        delta_x: isize,
        delta_y: isize,
        should_select: bool,
        gfx: &mut Gfx,
    ) {
        for index in self.cursor_indices() {
            self.move_cursor(index, delta_x, delta_y, should_select, gfx);
        }
    }

    pub fn move_cursor_to_next_word(
        &mut self,
        index: CursorIndex,
        delta_x: isize,
        should_select: bool,
        gfx: &mut Gfx,
    ) {
        let cursor = self.cursor(index);
        let destination = self.move_position_to_next_word(cursor.position, delta_x, gfx);

        self.jump_cursor(index, destination, should_select, gfx);
    }

    pub fn move_cursors_to_next_word(
        &mut self,
        delta_x: isize,
        should_select: bool,
        gfx: &mut Gfx,
    ) {
        for index in self.cursor_indices() {
            self.move_cursor_to_next_word(index, delta_x, should_select, gfx);
        }
    }

    pub fn move_cursor_to_next_paragraph(
        &mut self,
        index: CursorIndex,
        delta_y: isize,
        should_select: bool,
        gfx: &mut Gfx,
    ) {
        let cursor = self.cursor(index);
        let destination = self.move_position_to_next_paragraph(cursor.position, delta_y, true, gfx);

        self.jump_cursor(index, destination, should_select, gfx);
    }

    pub fn move_cursors_to_next_paragraph(
        &mut self,
        delta_y: isize,
        should_select: bool,
        gfx: &mut Gfx,
    ) {
        for index in self.cursor_indices() {
            self.move_cursor_to_next_paragraph(index, delta_y, should_select, gfx);
        }
    }

    pub fn jump_cursor(
        &mut self,
        index: CursorIndex,
        position: Position,
        should_select: bool,
        gfx: &mut Gfx,
    ) {
        self.update_cursor_selection(index, should_select);
        self.cursor_mut(index).position = self.clamp_position(position);
        self.update_cursor_desired_visual_x(index, gfx);
    }

    pub fn jump_cursors(&mut self, position: Position, should_select: bool, gfx: &mut Gfx) {
        self.clear_extra_cursors(CursorIndex::Main);
        self.jump_cursor(CursorIndex::Main, position, should_select, gfx);
    }

    pub fn start_cursor_selection(&mut self, index: CursorIndex) {
        let position = self.cursor(index).position;
        self.cursor_mut(index).selection_anchor = Some(position);
    }

    pub fn end_cursor_selection(&mut self, index: CursorIndex) {
        self.cursor_mut(index).selection_anchor = None;
    }

    pub fn update_cursor_selection(&mut self, index: CursorIndex, should_select: bool) {
        let cursor = self.cursor(index);

        if should_select && cursor.selection_anchor.is_none() {
            self.start_cursor_selection(index);
        } else if !should_select && cursor.selection_anchor.is_some() {
            self.end_cursor_selection(index);
        }
    }

    fn cursor_visual_x(&self, index: CursorIndex, gfx: &mut Gfx) -> usize {
        let cursor = self.cursor(index);
        let leading_text = &self.lines[cursor.position.y][..cursor.position.x];

        gfx.measure_text(leading_text)
    }

    fn update_cursor_desired_visual_x(&mut self, index: CursorIndex, gfx: &mut Gfx) {
        if !self.flags.contains(DocFlag::UpdateCursors) {
            return;
        }

        self.cursor_mut(index).desired_visual_x = self.cursor_visual_x(index, gfx);
    }

    pub fn add_cursor_at(&mut self, position: Position, gfx: &mut Gfx) {
        self.add_cursor(Cursor::new(position, 0));
        self.update_cursor_desired_visual_x(CursorIndex::Main, gfx);
    }

    pub fn add_cursor(&mut self, mut cursor: Cursor) {
        cursor.position = self.clamp_position(cursor.position);

        self.cursors.push(cursor);
    }

    pub fn unwrap_cursor_index(&self, index: CursorIndex) -> usize {
        let main_cursor_index = self.main_cursor_index();

        index.unwrap_or(main_cursor_index)
    }

    pub fn remove_cursor(&mut self, index: CursorIndex) {
        if self.cursors.len() < 2 {
            return;
        }

        let index = self.unwrap_cursor_index(index);
        self.cursors.remove(index);
    }

    pub fn cursor(&self, index: CursorIndex) -> &Cursor {
        let index = self.unwrap_cursor_index(index);
        &self.cursors[index]
    }

    fn cursor_mut(&mut self, index: CursorIndex) -> &mut Cursor {
        let index = self.unwrap_cursor_index(index);
        &mut self.cursors[index]
    }

    pub fn set_cursor_selection(&mut self, index: CursorIndex, selection: Option<Selection>) {
        let cursor = self.cursor_mut(index);

        let Some(selection) = selection else {
            cursor.selection_anchor = None;
            return;
        };

        let is_cursor_at_start = if let Some(current_selection) = cursor.get_selection() {
            cursor.position == current_selection.start
        } else {
            false
        };

        if is_cursor_at_start {
            cursor.selection_anchor = Some(selection.end);
            cursor.position = selection.start;
        } else {
            cursor.selection_anchor = Some(selection.start);
            cursor.position = selection.end;
        }
    }

    pub fn cursor_indices(&self) -> CursorIndices {
        CursorIndices::new(0, self.cursors.len())
    }

    pub fn cursors_len(&self) -> usize {
        self.cursors.len()
    }

    fn main_cursor_index(&self) -> usize {
        self.cursors.len() - 1
    }

    pub fn lines(&self) -> &[Pooled<String>] {
        &self.lines
    }

    pub fn collect_string(&self, start: Position, end: Position, result: &mut String) {
        let start = self.clamp_position(start);
        let end = self.clamp_position(end);

        if start.y == end.y {
            result.push_str(&self.lines[start.y][start.x..end.x]);
        } else {
            result.push_str(&self.lines[start.y][start.x..]);
            result.push('\n');

            for line in &self.lines[start.y + 1..end.y] {
                result.push_str(&line[..]);
                result.push('\n');
            }

            result.push_str(&self.lines[end.y][..end.x]);
        }
    }

    pub fn undo(&mut self, action_kind: ActionKind, ctx: &mut Ctx) {
        let mut last_popped_time = None;
        let mut were_cursors_reset = false;

        let reverse_action_kind = action_kind.reverse();

        while let Some(popped_action) = action_history!(self, action_kind).pop(last_popped_time) {
            last_popped_time = Some(popped_action.time);

            match popped_action.action {
                Action::SetCursor {
                    index,
                    position,
                    selection_anchor,
                } => {
                    if !were_cursors_reset {
                        self.reset_cursors();
                        were_cursors_reset = true;
                    }

                    if self.cursors.len() <= index {
                        self.cursors
                            .resize(index + 1, Cursor::new(Position::ZERO, 0));
                    }

                    let mut cursor = Cursor::new(position, 0);
                    cursor.selection_anchor = selection_anchor;

                    self.cursors[index] = cursor;
                    self.update_cursor_desired_visual_x(CursorIndex::Some(index), ctx.gfx);
                }
                Action::Insert { start, end } => {
                    were_cursors_reset = false;

                    self.delete_as_action_kind(
                        start,
                        end,
                        reverse_action_kind,
                        ctx_with_time!(ctx, popped_action.time),
                    );
                }
                Action::Delete { start, text_start } => {
                    were_cursors_reset = false;

                    let mut undone_text = STRING_POOL.new_item();

                    undone_text
                        .push_str(&action_history!(self, action_kind).deleted_text[text_start..]);

                    self.insert_as_action_kind(
                        start,
                        &undone_text,
                        reverse_action_kind,
                        ctx_with_time!(ctx, popped_action.time),
                    );

                    action_history!(self, action_kind)
                        .deleted_text
                        .truncate(text_start);
                }
            }
        }
    }

    pub fn add_cursors_to_action_history(&mut self, action_kind: ActionKind, time: f32) {
        if !self.flags.contains(DocFlag::TrackHistory) {
            return;
        }

        for index in self.cursor_indices() {
            let Cursor {
                position,
                selection_anchor,
                ..
            } = *self.cursor(index);

            let index = self.unwrap_cursor_index(index);

            action_history!(self, action_kind).push_set_cursor(
                index,
                position,
                selection_anchor,
                time,
            );
        }
    }

    pub fn clear_highlights(&mut self) {
        self.unhighlighted_line_y = 0;
        self.syntax_highlighter.clear();
    }

    fn mark_line_dirty(&mut self, y: usize) {
        self.unhighlighted_line_y = self.unhighlighted_line_y.min(y);
        self.needs_tokenization = true;
    }

    pub fn delete(&mut self, start: Position, end: Position, ctx: &mut Ctx) {
        self.delete_as_action_kind(start, end, ActionKind::Done, ctx);
    }

    pub fn delete_as_action_kind(
        &mut self,
        start: Position,
        end: Position,
        action_kind: ActionKind,
        ctx: &mut Ctx,
    ) {
        let start = self.clamp_position(start);
        let end = self.clamp_position(end);

        if start == end {
            return;
        }

        if action_kind == ActionKind::Done {
            self.redo_history.clear();
        }

        self.mark_line_dirty(start.y);
        self.is_saved = false;
        self.version += 1;
        self.lsp_did_change(start, end, "", ctx);

        if self.do_shift() {
            self.add_cursors_to_action_history(action_kind, ctx.time);
        }

        if self.flags.contains(DocFlag::TrackHistory) {
            let mut undone_text = STRING_POOL.new_item();
            self.collect_string(start, end, &mut undone_text);

            let deleted_text_start = action_history!(self, action_kind).deleted_text.len();

            action_history!(self, action_kind)
                .deleted_text
                .push_str(&undone_text);

            action_history!(self, action_kind).push_delete(start, deleted_text_start, ctx.time);
        }

        if start.y == end.y {
            self.lines[start.y].drain(start.x..end.x);
        } else {
            let (start_lines, end_lines) = self.lines.split_at_mut(end.y);

            let start_line = &mut start_lines[start.y];
            let end_line = end_lines.first().unwrap();

            start_line.truncate(start.x);
            start_line.push_str(&end_line[end.x..]);

            self.lines.remove(end.y);
            self.lines.drain(start.y + 1..end.y);
        }

        if self.do_shift() {
            self.shift_positions(start, end, Self::shift_position_by_delete, ctx);
        }

        self.lsp_diagnostic(ctx);
    }

    pub fn insert(&mut self, start: Position, text: &str, ctx: &mut Ctx) -> Position {
        self.insert_as_action_kind(start, text, ActionKind::Done, ctx)
    }

    pub fn insert_as_action_kind(
        &mut self,
        start: Position,
        text: &str,
        action_kind: ActionKind,
        ctx: &mut Ctx,
    ) -> Position {
        if text.is_empty() {
            return start;
        }

        if action_kind == ActionKind::Done {
            self.redo_history.clear();
        }

        self.mark_line_dirty(start.y);
        self.is_saved = false;
        self.version += 1;
        self.lsp_did_change(start, start, text, ctx);

        if self.do_shift() {
            self.add_cursors_to_action_history(action_kind, ctx.time);
        }

        let start = self.clamp_position(start);
        let mut position = self.clamp_position(start);

        for grapheme in GraphemeIterator::new(text) {
            match grapheme {
                "\r" => continue,
                "\r\n" | "\n" => {
                    if !self.flags.contains(DocFlag::AllowMultipleLines) {
                        continue;
                    }

                    let new_y = position.y + 1;
                    let split_x = position.x;

                    position.y += 1;
                    position.x = 0;

                    self.lines.insert(new_y, STRING_POOL.new_item());

                    let (old, new) = self.lines.split_at_mut(new_y);

                    let old = old.last_mut().unwrap();
                    let new = new.first_mut().unwrap();

                    new.push_str(&old[split_x..]);
                    old.truncate(split_x);

                    continue;
                }
                _ => {}
            }

            self.lines[position.y].insert_str(position.x, grapheme);
            position.x += grapheme.len();
        }

        if self.flags.contains(DocFlag::TrackHistory) {
            action_history!(self, action_kind).push_insert(start, position, ctx.time);
        }

        if self.do_shift() {
            self.shift_positions(start, position, Self::shift_position_by_insert, ctx);
        }

        self.lsp_diagnostic(ctx);

        position
    }

    fn shift_positions(
        &mut self,
        start: Position,
        end: Position,
        shift: fn(&Self, Position, Position, Position) -> Position,
        ctx: &mut Ctx,
    ) {
        for index in self.cursor_indices() {
            let cursor = self.cursor(index);

            let position = shift(self, start, end, cursor.position);
            let selection_anchor = cursor
                .selection_anchor
                .map(|selection_anchor| shift(self, start, end, selection_anchor));

            let cursor = self.cursor_mut(index);

            cursor.position = position;
            cursor.selection_anchor = selection_anchor;

            self.update_cursor_desired_visual_x(index, ctx.gfx);
        }

        for language_server in ctx.lsp.iter_servers_mut() {
            for DecodedDiagnostic { range, .. } in language_server.diagnostics_mut(self) {
                range.start = shift(self, start, end, range.start);
                range.end = shift(self, start, end, range.end);
            }
        }
    }

    pub fn shift_position_by_insert(
        &self,
        start: Position,
        end: Position,
        position: Position,
    ) -> Position {
        if start.y == position.y && start.x <= position.x {
            Position::new(position.x + end.x - start.x, position.y + end.y - start.y)
        } else if start.y < position.y {
            Position::new(position.x, position.y + end.y - start.y)
        } else {
            position
        }
    }

    pub fn shift_position_by_delete(
        &self,
        start: Position,
        end: Position,
        position: Position,
    ) -> Position {
        let influence_end = end.min(position);

        if influence_end <= start {
            return position;
        }

        if influence_end.y == position.y && influence_end.x <= position.x {
            // Use isize to allow for wrapping the position when deleting at the start of a line.
            let x = (position.x as isize - (influence_end.x as isize - start.x as isize)) as usize;

            Position::new(x, position.y - (influence_end.y - start.y))
        } else if influence_end.y < position.y {
            Position::new(position.x, position.y - (influence_end.y - start.y))
        } else {
            position
        }
    }

    pub fn insert_at_cursors(&mut self, text: &str, ctx: &mut Ctx) {
        for index in self.cursor_indices() {
            self.insert_at_cursor(index, text, ctx);
        }
    }

    pub fn insert_at_cursor(&mut self, index: CursorIndex, text: &str, ctx: &mut Ctx) {
        if let Some(selection) = self.cursor(index).get_selection() {
            self.delete(selection.start, selection.end, ctx);
            self.end_cursor_selection(index);
        }

        let start = self.cursor(index).position;
        self.insert(start, text, ctx);
    }

    pub fn search(
        &self,
        text: &str,
        start: Position,
        is_reverse: bool,
        gfx: &mut Gfx,
    ) -> Option<Position> {
        if is_reverse {
            self.search_backward(text, start, true, gfx)
        } else {
            self.search_forward(text, start, true, gfx)
        }
    }

    pub fn search_forward(
        &self,
        text: &str,
        start: Position,
        do_wrap: bool,
        gfx: &mut Gfx,
    ) -> Option<Position> {
        let start = self.move_position(start, 1, 0, gfx);

        if text.is_empty() {
            return Some(start);
        }

        let mut y = start.y as isize;
        let mut x = start.x;

        let mut match_cursor = CharCursor::new(0, text.len());

        loop {
            let line = &self.lines[y as usize];

            for c in CharIterator::with_offset(x, line) {
                for _ in 0..2 {
                    if grapheme::char_at(match_cursor.index(), text) == c {
                        match_cursor.next_boundary(text);

                        if match_cursor.index() >= text.len() {
                            let match_x =
                                c.as_ptr() as usize + c.len() - line.as_ptr() as usize - text.len();

                            return Some(Position::new(match_x, y as usize));
                        }

                        break;
                    } else if match_cursor.index() > 0 {
                        match_cursor.set_index(0);

                        // Now retry matching from the start of the text.
                        continue;
                    }

                    break;
                }
            }

            y += 1;

            if y >= self.lines.len() as isize {
                if do_wrap {
                    y = 0;
                } else {
                    break;
                }
            }

            if y == start.y as isize {
                break;
            }

            x = 0;
        }

        None
    }

    pub fn search_backward(
        &self,
        text: &str,
        start: Position,
        do_wrap: bool,
        gfx: &mut Gfx,
    ) -> Option<Position> {
        let start = self.move_position(start, -1, 0, gfx);

        if text.is_empty() {
            return Some(start);
        }

        let mut y = start.y as isize;
        let mut x = start.x;

        let mut match_cursor = CharCursor::new(text.len(), text.len());

        loop {
            let line = &self.lines[y as usize];

            for c in CharIterator::with_offset(x, line).rev() {
                for _ in 0..2 {
                    if grapheme::char_ending_at(match_cursor.index(), text) == c {
                        match_cursor.previous_boundary(text);

                        if match_cursor.index() == 0 {
                            let match_x = c.as_ptr() as usize - line.as_ptr() as usize;

                            return Some(Position::new(match_x, y as usize));
                        }

                        break;
                    } else if match_cursor.index() < text.len() {
                        match_cursor.set_index(text.len());

                        // Now retry matching from the start of the text.
                        continue;
                    }

                    break;
                }
            }

            y -= 1;

            if y < 0 {
                if do_wrap {
                    y = self.lines.len() as isize - 1;
                } else {
                    break;
                }
            }

            if y == start.y as isize {
                break;
            }

            x = self.lines[y as usize].len();
        }

        None
    }

    pub fn end(&self) -> Position {
        self.line_end(self.lines().len() - 1)
    }

    pub fn grapheme(&self, position: Position) -> &str {
        let position = self.clamp_position(position);
        let line = &self.lines[position.y];

        if position.x == line.len() {
            "\n"
        } else {
            grapheme::at(position.x, line)
        }
    }

    // It's ok for the x position to equal the length of the line.
    // That represents the cursor being right before the newline sequence.
    fn clamp_position(&self, position: Position) -> Position {
        let max_y = self.lines.len() - 1;
        let clamped_y = position.y.clamp(0, max_y);

        let max_x = self.lines[clamped_y].len();
        let clamped_x = position.x.clamp(0, max_x);

        Position::new(clamped_x, clamped_y)
    }

    pub fn position_to_visual(
        &self,
        position: Position,
        camera_position: VisualPosition,
        gfx: &mut Gfx,
    ) -> VisualPosition {
        let position = self.clamp_position(position);
        let leading_text = &self.lines[position.y][..position.x];

        let visual_x = gfx.line_padding_x()
            + gfx.measure_text(leading_text) as f32 * gfx.glyph_width()
            - camera_position.x;

        let visual_y = position.y as f32 * gfx.line_height() - camera_position.y;

        VisualPosition::new(visual_x, visual_y)
    }

    pub fn visual_to_position(
        &self,
        visual: VisualPosition,
        camera_position: VisualPosition,
        gfx: &mut Gfx,
    ) -> Position {
        let mut position = self.visual_to_position_with_visual_x(visual, camera_position, gfx);
        let visual_x = position.x;

        position = self.clamp_position(position);
        position.x = gfx.find_x_for_visual_x(&self.lines[position.y], visual_x);

        position
    }

    pub fn visual_to_position_unclamped(
        &self,
        visual: VisualPosition,
        camera_position: VisualPosition,
        gfx: &mut Gfx,
    ) -> Option<Position> {
        if visual.y < 0.0 {
            return None;
        }

        let mut position = self.visual_to_position_with_visual_x(visual, camera_position, gfx);

        if position.y >= self.lines.len() {
            return None;
        }

        position.x = gfx.find_x_for_visual_x_unclamped(&self.lines[position.y], position.x)?;

        Some(position)
    }

    fn visual_to_position_with_visual_x(
        &self,
        visual: VisualPosition,
        camera_position: VisualPosition,
        gfx: &Gfx,
    ) -> Position {
        Position::new(
            ((visual.x + camera_position.x) / gfx.glyph_width()).max(0.0) as usize,
            ((visual.y + camera_position.y) / gfx.line_height()).max(0.0) as usize,
        )
    }

    pub fn trim_trailing_whitespace(&mut self, ctx: &mut Ctx) {
        for y in 0..self.lines.len() {
            self.trim_trailing_whitespace_at(y, ctx);
        }
    }

    pub fn trim_trailing_whitespace_at(&mut self, y: usize, ctx: &mut Ctx) {
        let line = &self.lines[y];
        let mut whitespace_start = 0;

        let mut grapheme_cursor = GraphemeCursor::new(line.len(), line.len());

        loop {
            let grapheme = grapheme::at(grapheme_cursor.index(), line);

            if !grapheme::is_whitespace(grapheme) {
                break;
            };

            whitespace_start = grapheme_cursor.index();

            if grapheme_cursor.previous_boundary(line).is_none() {
                break;
            }
        }

        if whitespace_start < line.len() {
            let start = Position::new(whitespace_start, y);
            let end = Position::new(line.len(), y);

            self.delete(start, end, ctx);
        }
    }

    fn reset_cursors(&mut self) {
        self.cursors.clear();
        self.cursors.push(Cursor::new(Position::ZERO, 0));
    }

    pub fn clear_extra_cursors(&mut self, kept_index: CursorIndex) {
        let kept_index = self.unwrap_cursor_index(kept_index);

        self.cursors.swap(0, kept_index);
        self.cursors.truncate(1);
    }

    fn reset_edit_state(&mut self) {
        self.undo_history.clear();
        self.redo_history.clear();

        self.reset_cursors();

        self.is_saved = true;
        self.version = 0;
    }

    pub fn clear(&mut self, ctx: &mut Ctx) {
        self.line_ending = LineEnding::default();

        self.mark_line_dirty(0);
        self.reset_edit_state();

        self.lines.push(STRING_POOL.new_item());
        self.lines.drain(..self.lines.len() - 1);

        self.lsp_did_close(ctx);
    }

    pub fn is_change_unexpected(&mut self) -> bool {
        if self.expected_change_count == 0 {
            true
        } else {
            self.expected_change_count -= 1;

            false
        }
    }

    pub fn save(&mut self, path: Option<Pooled<PathBuf>>, ctx: &mut Ctx) -> io::Result<()> {
        if self.is_saved
            && path
                .as_ref()
                .is_none_or(|path| Some(path) == self.path.some())
        {
            return Ok(());
        }

        let string = self.to_string();

        if let Some(path) = path {
            self.set_path_on_drive(path)?;
        }

        let Some(path) = self.path.some() else {
            return Ok(());
        };

        File::create(path)?.write_all(string.as_bytes())?;

        self.path = match take(&mut self.path) {
            DocPath::None => DocPath::None,
            DocPath::InMemory(path) => DocPath::OnDrive(path),
            DocPath::OnDrive(path) => {
                self.expected_change_count = EXPECTED_CHANGE_COUNT_ON_SAVE;

                DocPath::OnDrive(path)
            }
        };

        self.is_saved = true;
        self.lsp_text_document_notification("textDocument/didSave", ctx);

        Ok(())
    }

    pub fn load(&mut self, ctx: &mut Ctx) -> io::Result<()> {
        self.clear(ctx);

        let Some(path) = self.path.some() else {
            return Ok(());
        };

        let string = read_to_string(path)?;

        let (line_ending, len) = self.line_ending_and_len(&string);

        self.insert(Position::ZERO, &string[..len], ctx);
        self.reset_edit_state();
        self.line_ending = line_ending;

        self.path = match take(&mut self.path) {
            DocPath::None => DocPath::None,
            DocPath::InMemory(path) => DocPath::OnDrive(path),
            DocPath::OnDrive(path) => DocPath::OnDrive(path),
        };

        self.lsp_did_open(&string, ctx);

        Ok(())
    }

    pub fn reload(&mut self, ctx: &mut Ctx) -> io::Result<()> {
        let Some(path) = self.path.on_drive() else {
            return Ok(());
        };

        let string = read_to_string(path)?;

        self.add_cursors_to_action_history(ActionKind::Done, ctx.time);

        self.do_skip_shifting = true;

        self.delete(Position::ZERO, self.end(), ctx);

        let (line_ending, len) = self.line_ending_and_len(&string);

        self.line_ending = line_ending;
        self.insert(Position::ZERO, &string[..len], ctx);

        self.do_skip_shifting = false;

        self.shift_positions(
            Position::ZERO,
            Position::ZERO,
            |doc, _, _, position| doc.clamp_position(position),
            ctx,
        );

        self.is_saved = true;

        Ok(())
    }

    fn line_ending_and_len(&self, string: &str) -> (LineEnding, usize) {
        let mut len = 0;
        let mut grapheme_iterator = GraphemeIterator::new(string);

        for grapheme in grapheme_iterator.by_ref() {
            match grapheme {
                "\r\n" | "\r" | "\n" => {
                    let line_ending = if grapheme == "\r\n"
                        || (grapheme == "\r" && grapheme_iterator.next() == Some("\n"))
                    {
                        LineEnding::CrLf
                    } else {
                        LineEnding::Lf
                    };

                    let len = if self.flags.contains(DocFlag::AllowMultipleLines) {
                        string.len()
                    } else {
                        len
                    };

                    return (line_ending, len);
                }
                _ => {}
            }

            len += grapheme.len();
        }

        (LineEnding::default(), string.len())
    }

    fn set_path_on_drive(&mut self, path: Pooled<PathBuf>) -> io::Result<()> {
        self.path = DocPath::OnDrive(if path.is_normal() {
            path
        } else {
            path.normalized()?
        });

        Ok(())
    }

    pub fn path(&self) -> &DocPath {
        &self.path
    }

    pub fn set_display_name(&mut self, display_name: Option<Pooled<String>>) {
        self.display_name = display_name;
    }

    pub fn file_name(&self) -> &str {
        const DEFAULT_NAME: &str = "Unnamed";

        self.path
            .some()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .or(self
                .display_name
                .as_ref()
                .map(|display_name| display_name.as_str()))
            .unwrap_or(DEFAULT_NAME)
    }

    pub fn is_saved(&self) -> bool {
        self.is_saved || !self.flags.contains(DocFlag::TrackSaving)
    }

    pub fn is_worthless(&self) -> bool {
        self.path.is_none() && self.is_saved()
    }

    pub fn has_selection(&self) -> bool {
        for index in self.cursor_indices() {
            if self.cursor(index).get_selection().is_some() {
                return true;
            }
        }

        false
    }

    fn do_shift(&self) -> bool {
        !self.do_skip_shifting && self.flags.contains(DocFlag::UpdateCursors)
    }

    pub fn flags(&self) -> DocFlags {
        self.flags
    }

    pub fn copy_at_cursors(&self, text: &mut String) -> bool {
        let mut was_copy_implicit = true;

        for index in self.cursor_indices() {
            let cursor = self.cursor(index);

            if let Some(selection) = cursor.get_selection() {
                was_copy_implicit = false;

                self.collect_string(selection.start, selection.end, text);
            } else {
                self.copy_line_at_position(cursor.position, text);
            }

            if self.unwrap_cursor_index(index) != self.cursors_len() - 1 {
                text.push('\n');
            }
        }

        was_copy_implicit
    }

    pub fn copy_line_at_position(&self, position: Position, text: &mut String) {
        let start = Position::new(0, position.y);
        let end = self.line_end(start.y);

        self.collect_string(start, end, text);
        text.push('\n');
    }

    pub fn paste_at_cursor(
        &mut self,
        index: CursorIndex,
        text: &str,
        was_copy_implicit: bool,
        ctx: &mut Ctx,
    ) {
        let mut start = self.cursor(index).position;

        if let Some(selection) = self.cursor(index).get_selection() {
            self.delete(selection.start, selection.end, ctx);
            self.end_cursor_selection(index);

            start = selection.start;
        } else if was_copy_implicit {
            start.x = 0;
        }

        self.insert(start, text, ctx);
    }

    pub fn paste_at_cursors(&mut self, text: &str, was_copy_implicit: bool, ctx: &mut Ctx) {
        let mut line_count = 1;

        for c in text.chars() {
            if c == '\n' {
                line_count += 1;
            }
        }

        let do_spread_lines_between_cursors =
            self.cursors_len() > 1 && line_count % self.cursors_len() == 0;

        if do_spread_lines_between_cursors {
            let lines_per_cursor = line_count / self.cursors_len();
            let mut grapheme_iterator = GraphemeIterator::new(text);

            for index in self.cursor_indices() {
                for line_i in 0..lines_per_cursor {
                    for grapheme in grapheme_iterator.by_ref() {
                        if grapheme == "\n" {
                            if line_i < lines_per_cursor - 1 {
                                self.paste_at_cursor(index, grapheme, was_copy_implicit, ctx);
                            }

                            break;
                        }

                        self.paste_at_cursor(index, grapheme, was_copy_implicit, ctx);
                    }
                }
            }
        } else {
            for index in self.cursor_indices() {
                self.paste_at_cursor(index, text, was_copy_implicit, ctx);
            }
        }
    }

    pub fn update_tokens(&mut self) {
        if !self.needs_tokenization {
            return;
        }

        self.needs_tokenization = false;
        self.tokenizer.tokenize(&self.lines);
    }

    pub fn tokens(&self) -> &Trie {
        self.tokenizer.tokens()
    }

    pub fn update_highlights(
        &mut self,
        camera_position: VisualPosition,
        bounds: Rect,
        syntax: &Syntax,
        gfx: &mut Gfx,
    ) {
        let end = self.visual_to_position(
            VisualPosition::new(0.0, camera_position.y + bounds.height),
            camera_position,
            gfx,
        );

        self.syntax_highlighter
            .update(&self.lines, syntax, self.unhighlighted_line_y, end.y);

        self.unhighlighted_line_y = end.y + 1;
    }

    pub fn scroll_highlighted_lines(&mut self, region: RangeInclusive<usize>, delta_y: isize) {
        self.syntax_highlighter
            .scroll_highlighted_lines(region, delta_y);
    }

    pub fn highlight_line_from_terminal_colors(
        &mut self,
        colors: &[(TerminalHighlightKind, TerminalHighlightKind)],
        y: usize,
    ) {
        self.syntax_highlighter
            .highlight_line_from_terminal_colors(colors, y);
    }

    pub fn highlighted_lines(&self) -> &[HighlightedLine] {
        self.syntax_highlighter.highlighted_lines()
    }

    pub fn combine_overlapping_cursors(&mut self) {
        for index in self.cursor_indices().rev() {
            let cursor = self.cursor(index);
            let position = cursor.position;
            let selection = cursor.get_selection();

            for other_index in self.cursor_indices() {
                if self.unwrap_cursor_index(index) == self.unwrap_cursor_index(other_index) {
                    continue;
                }

                let other_cursor = self.cursor(other_index);

                let do_remove = if let Some(selection) = other_cursor.get_selection() {
                    position >= selection.start && position <= selection.end
                } else {
                    position == other_cursor.position
                };

                if !do_remove {
                    continue;
                }

                self.set_cursor_selection(
                    other_index,
                    Selection::union(other_cursor.get_selection(), selection),
                );
                self.remove_cursor(index);

                break;
            }
        }
    }

    pub fn add_cursor_at_next_occurance(&mut self, gfx: &mut Gfx) {
        let cursor = self.cursor(CursorIndex::Main);

        let Some(selection) = cursor.get_selection() else {
            self.select_current_word_at_cursors(gfx);
            return;
        };

        if selection.start.y != selection.end.y {
            return;
        }

        let Some(line) = self.get_line(cursor.position.y) else {
            return;
        };

        let Some(position) = self.search(
            &line[selection.start.x..selection.end.x],
            cursor.position,
            false,
            gfx,
        ) else {
            return;
        };

        self.add_cursor_at(position, gfx);

        let end = self.move_position(
            position,
            selection.end.x as isize - selection.start.x as isize,
            0,
            gfx,
        );

        self.jump_cursor(CursorIndex::Main, end, true, gfx);
    }

    pub fn select_current_line_at_position(&self, position: Position, gfx: &mut Gfx) -> Selection {
        let mut start = Position::new(0, position.y);
        let mut end = self.line_end(start.y);

        if start.y == self.lines().len() - 1 {
            start = self.move_position(start, -1, 0, gfx);
        } else {
            end = self.move_position(end, 1, 0, gfx);
        }

        Selection { start, end }
    }

    pub fn select_current_line_at_cursors(&mut self, gfx: &mut Gfx) {
        for index in self.cursor_indices() {
            let position = self.cursor(index).position;
            let selection = self.select_current_line_at_position(position, gfx);

            let cursor = self.cursor_mut(index);

            cursor.selection_anchor = Some(selection.start);
            cursor.position = selection.end;
        }
    }

    pub fn select_current_word_at_position(
        &self,
        mut position: Position,
        gfx: &mut Gfx,
    ) -> Selection {
        let line_len = self.line_len(position.y);

        if position.x < line_len {
            position = self.move_position(position, 1, 0, gfx);
        }

        if position.x > 0 {
            position = self.move_position_to_next_word(position, -1, gfx);
        }

        let start = position;

        if position.x < line_len {
            position = self.move_position_to_next_word(position, 1, gfx);
        }

        let end = position;

        Selection { start, end }
    }

    pub fn select_current_word_at_cursors(&mut self, gfx: &mut Gfx) {
        for index in self.cursor_indices() {
            let position = self.cursor(index).position;
            let selection = self.select_current_word_at_position(position, gfx);

            let cursor = self.cursor_mut(index);

            cursor.selection_anchor = Some(selection.start);
            cursor.position = selection.end;
        }
    }

    pub fn get_completion_prefix<'a>(&'a self, gfx: &mut Gfx) -> Option<&'a str> {
        let prefix_end = self.cursor(CursorIndex::Main).position;

        if prefix_end.x == 0 {
            return None;
        }

        let mut prefix_start = prefix_end;

        while prefix_start.x > 0 {
            let next_start = self.move_position(prefix_start, -1, 0, gfx);

            let grapheme = self.grapheme(next_start);

            if grapheme::is_alphanumeric(grapheme) || grapheme == "_" {
                prefix_start = next_start;
                continue;
            }

            // These characters aren't included in the completion prefix
            // but they should still trigger a completion.
            if !matches!(grapheme, "." | ":") && prefix_start == prefix_end {
                return None;
            }

            break;
        }

        self.get_line(prefix_end.y)
            .map(|line| &line[prefix_start.x..prefix_end.x])
    }

    pub fn version(&self) -> usize {
        self.version
    }

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }
}

impl Display for Doc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let line_ending_str = match self.line_ending {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
        };

        for (i, line) in self.lines.iter().enumerate() {
            f.write_str(&line[..])?;

            if i != self.lines.len() - 1 {
                f.write_str(line_ending_str)?;
            }
        }

        Ok(())
    }
}
