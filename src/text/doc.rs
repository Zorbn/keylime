use std::{
    borrow::Borrow,
    fmt::{Display, Write as _},
    fs::{read_to_string, File},
    io::{self, Write},
    path::{absolute, Path, PathBuf},
    vec::Drain,
};

use crate::{
    config::language::IndentWidth,
    editor_buffers::EditorBuffers,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    platform::gfx::Gfx,
};

use super::{
    action_history::{Action, ActionHistory, ActionKind},
    char_category::CharCategory,
    cursor::Cursor,
    cursor_index::{CursorIndex, CursorIndices},
    line_pool::{Line, LinePool},
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

#[derive(Default)]
enum LineEnding {
    Lf,
    #[default]
    CrLf,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum DocKind {
    MultiLine,
    SingleLine,
    Output,
}

enum StepStatus {
    None,
    Wrapped,
    Done,
}

const CURSOR_POSITION_HISTORY_THRESHOLD: isize = 10;

// One change for File::create, and one change for writing.
#[cfg(target_os = "windows")]
const EXPECTED_CHANGE_COUNT_ON_SAVE: usize = 2;

#[cfg(target_os = "macos")]
const EXPECTED_CHANGE_COUNT_ON_SAVE: usize = 1;

pub struct Doc {
    display_name: Option<&'static str>,
    path: Option<PathBuf>,
    is_saved: bool,
    expected_change_count: usize,
    version: usize,
    usages: usize,

    lines: Vec<Line>,
    cursors: Vec<Cursor>,
    line_ending: LineEnding,

    undo_history: ActionHistory,
    redo_history: ActionHistory,
    undo_char_buffer: Option<Vec<char>>,

    cursor_position_undo_history: Vec<Position>,
    cursor_position_redo_history: Vec<Position>,

    syntax_highlighter: SyntaxHighlighter,
    unhighlighted_line_y: isize,
    tokenizer: Tokenizer,
    needs_tokenization: bool,

    kind: DocKind,
}

impl Doc {
    pub fn new(
        line_pool: &mut LinePool,
        display_name: Option<&'static str>,
        kind: DocKind,
    ) -> Self {
        let lines = vec![line_pool.pop()];

        let mut doc = Self {
            display_name,
            path: None,
            is_saved: true,
            expected_change_count: 0,
            version: 0,
            usages: 0,

            lines,
            cursors: Vec::new(),
            line_ending: LineEnding::default(),

            undo_history: ActionHistory::new(),
            redo_history: ActionHistory::new(),
            undo_char_buffer: Some(Vec::new()),

            cursor_position_undo_history: Vec::new(),
            cursor_position_redo_history: Vec::new(),

            syntax_highlighter: SyntaxHighlighter::new(),
            unhighlighted_line_y: 0,
            tokenizer: Tokenizer::new(),
            needs_tokenization: false,

            kind,
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

    pub fn move_position(&self, position: Position, delta: Position) -> Position {
        self.move_position_with_desired_visual_x(position, delta, None)
    }

    pub fn move_position_with_desired_visual_x(
        &self,
        position: Position,
        delta: Position,
        desired_visual_x: Option<isize>,
    ) -> Position {
        let position = self.clamp_position(position);

        let mut new_y = position.y + delta.y;
        let mut new_x = position.x;

        if delta.y != 0 {
            if new_y < 0 {
                return Position::new(0, 0);
            }

            if new_y >= self.lines.len() as isize {
                return Position::new(
                    self.lines.last().unwrap().len() as isize,
                    self.lines.len() as isize - 1,
                );
            }

            if let Some(desired_visual_x) = desired_visual_x {
                new_x = Gfx::find_x_for_visual_x(&self.lines[new_y as usize], desired_visual_x);
            } else if new_x > self.get_line_len(new_y) {
                new_x = self.get_line_len(new_y);
            }
        }

        new_x += delta.x;

        while new_x < 0 {
            if new_y == 0 {
                new_x = 0;
                break;
            }

            new_y -= 1;
            new_x += self.lines[new_y as usize].len() as isize + 1;
        }

        while new_x > self.get_line_len(new_y) {
            if new_y == self.lines.len() as isize - 1 {
                new_x = self.get_line_len(new_y);
                break;
            }

            new_x -= self.get_line_len(new_y) + 1;
            new_y += 1;
        }

        Position::new(new_x, new_y)
    }

    pub fn move_position_skipping_category(
        &self,
        position: Position,
        delta_x: isize,
        category: CharCategory,
    ) -> Position {
        let mut position = self.clamp_position(position);
        let side_offset = Self::get_side_offset(delta_x);
        let delta = Position::new(delta_x, 0);

        loop {
            let current_category =
                CharCategory::new(self.get_char(self.move_position(position, side_offset)));
            let next_position = self.move_position(position, delta);

            if current_category != category
                || current_category == CharCategory::Newline
                || next_position == position
            {
                break;
            }

            position = next_position;
        }

        position
    }

    pub fn move_position_to_next_word(&self, position: Position, delta_x: isize) -> Position {
        let starting_position =
            self.move_position_skipping_category(position, delta_x, CharCategory::Space);

        let side_offset = Self::get_side_offset(delta_x);
        let starting_category =
            CharCategory::new(self.get_char(self.move_position(starting_position, side_offset)));

        let ending_position =
            self.move_position_skipping_category(starting_position, delta_x, starting_category);

        if ending_position == position {
            self.move_position(ending_position, Position::new(delta_x, 0))
        } else {
            ending_position
        }
    }

    pub fn move_position_skipping_lines(
        &self,
        position: Position,
        delta_y: isize,
        do_skip_empty_lines: bool,
    ) -> Position {
        let mut position = self.clamp_position(position);
        let delta = Position::new(0, delta_y);

        loop {
            let current_line_is_empty = self.get_line_len(position.y) == 0;
            let next_position = self.move_position(position, delta);

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
    ) -> Position {
        let starting_position = if do_skip_leading_whitespace {
            self.move_position_skipping_lines(position, delta_y, true)
        } else {
            self.clamp_position(position)
        };

        let starting_line_is_empty = self.get_line_len(starting_position.y) == 0;

        self.move_position_skipping_lines(starting_position, delta_y, starting_line_is_empty)
    }

    fn get_side_offset(direction_x: isize) -> Position {
        if direction_x < 0 {
            Position::new(-1, 0)
        } else {
            Position::zero()
        }
    }

    pub fn get_line(&self, y: isize) -> Option<&Line> {
        if y < 0 || y >= self.lines.len() as isize {
            None
        } else {
            Some(&self.lines[y as usize])
        }
    }

    pub fn get_line_len(&self, y: isize) -> isize {
        if let Some(line) = self.get_line(y) {
            line.len() as isize
        } else {
            0
        }
    }

    pub fn get_line_start(&self, y: isize) -> isize {
        let Some(line) = self.get_line(y) else {
            return 0;
        };

        let mut start = 0;

        for c in line {
            if !c.is_whitespace() {
                break;
            }

            start += 1;
        }

        start
    }

    pub fn is_line_whitespace(&self, y: isize) -> bool {
        self.get_line_start(y) == self.get_line_len(y)
    }

    pub fn comment_line(
        &mut self,
        comment: &str,
        mut position: Position,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        for c in comment.chars() {
            self.insert(position, [c], line_pool, time);
            position = self.move_position(position, Position::new(1, 0));
        }

        self.insert(position, [' '], line_pool, time);
    }

    pub fn uncomment_line(
        &mut self,
        comment: &str,
        y: isize,
        line_pool: &mut LinePool,
        time: f32,
    ) -> bool {
        if !self.is_line_commented(comment, y) {
            return false;
        }

        let start = Position::new(self.get_line_start(y), y);
        let end = self.move_position(start, Position::new(comment.len() as isize, 0));

        self.delete(start, end, line_pool, time);

        if self.get_char(start) == ' ' {
            let end = self.move_position(start, Position::new(1, 0));

            self.delete(start, end, line_pool, time);
        }

        true
    }

    pub fn is_line_commented(&self, comment: &str, y: isize) -> bool {
        let Some(line) = self.get_line(y) else {
            return false;
        };

        let start = self.get_line_start(y) as usize;

        if start + comment.len() >= line.len() {
            return false;
        }

        for (i, c) in comment.chars().enumerate() {
            if line[start + i] != c {
                return false;
            }
        }

        true
    }

    pub fn toggle_comments_at_cursors(
        &mut self,
        comment: &str,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        for index in self.cursor_indices() {
            let cursor = self.get_cursor(index);

            let selection = cursor
                .get_selection()
                .unwrap_or(Selection {
                    start: cursor.position,
                    end: cursor.position,
                })
                .trim_lines_without_selected_chars();

            let mut min_comment_x = isize::MAX;
            let mut did_uncomment = false;

            for y in selection.start.y..=selection.end.y {
                if self.is_line_whitespace(y) {
                    continue;
                }

                min_comment_x = min_comment_x.min(self.get_line_start(y));

                did_uncomment = self.uncomment_line(comment, y, line_pool, time) || did_uncomment;
            }

            if did_uncomment {
                continue;
            }

            for y in selection.start.y..=selection.end.y {
                if self.is_line_whitespace(y) {
                    continue;
                }

                self.comment_line(comment, Position::new(min_comment_x, y), line_pool, time);
            }
        }
    }

    pub fn indent_lines_at_cursor(
        &mut self,
        index: CursorIndex,
        indent_width: IndentWidth,
        do_unindent: bool,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let cursor = self.get_cursor(index);

        let selection = cursor.get_selection();
        let has_selection = selection.is_some();

        let selection = selection
            .unwrap_or(Selection {
                start: cursor.position,
                end: cursor.position,
            })
            .trim_lines_without_selected_chars();

        for y in selection.start.y..=selection.end.y {
            if has_selection && self.get_line_len(y) == 0 {
                continue;
            }

            if do_unindent {
                self.unindent_line(y, indent_width, line_pool, time);
            } else {
                self.indent_line(y, indent_width, line_pool, time);
            }
        }
    }

    pub fn indent_lines_at_cursors(
        &mut self,
        indent_width: IndentWidth,
        do_unindent: bool,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        for index in self.cursor_indices() {
            self.indent_lines_at_cursor(index, indent_width, do_unindent, line_pool, time);
        }
    }

    fn indent_line(
        &mut self,
        y: isize,
        indent_width: IndentWidth,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        self.indent(Position::new(0, y), indent_width, line_pool, time);
    }

    fn unindent_line(
        &mut self,
        y: isize,
        indent_width: IndentWidth,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let end = Position::new(self.get_line_start(y), y);

        self.unindent(end, indent_width, line_pool, time);
    }

    fn indent(
        &mut self,
        mut start: Position,
        indent_width: IndentWidth,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        for c in indent_width.chars() {
            self.insert(start, [c], line_pool, time);
            start = self.move_position(start, Position::new(1, 0));
        }
    }

    fn unindent(
        &mut self,
        end: Position,
        indent_width: IndentWidth,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let start = self.get_indent_start(end, indent_width);

        self.delete(start, end, line_pool, time);
    }

    pub fn get_indent_start(&mut self, end: Position, indent_width: IndentWidth) -> Position {
        let mut start = self.move_position(end, Position::new(-1, 0));
        let start_char = self.get_char(start);

        match start_char {
            ' ' => {
                let indent_width = (end.x - 1) % indent_width.char_count() as isize + 1;

                for _ in 1..indent_width {
                    let next_start = self.move_position(start, Position::new(-1, 0));

                    if self.get_char(next_start) != ' ' {
                        break;
                    }

                    start = next_start;
                }

                start
            }
            '\t' => start,
            _ => end,
        }
    }

    pub fn indent_at_cursor(
        &mut self,
        index: CursorIndex,
        indent_width: IndentWidth,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let start = self.get_cursor(index).position;
        self.indent(start, indent_width, line_pool, time);
    }

    pub fn indent_at_cursors(
        &mut self,
        indent_width: IndentWidth,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        for index in self.cursor_indices() {
            self.indent_at_cursor(index, indent_width, line_pool, time);
        }
    }

    pub fn undo_cursor_position(&mut self) {
        let Some(position) = self.cursor_position_undo_history.pop() else {
            return;
        };

        let cursor = self.get_cursor(CursorIndex::Main);
        self.cursor_position_redo_history.push(cursor.position);

        let cursor = self.get_cursor_mut(CursorIndex::Main);
        cursor.position = position;
    }

    pub fn redo_cursor_position(&mut self) {
        let Some(position) = self.cursor_position_redo_history.pop() else {
            return;
        };

        let cursor = self.get_cursor(CursorIndex::Main);
        self.cursor_position_undo_history.push(cursor.position);

        let cursor = self.get_cursor_mut(CursorIndex::Main);
        cursor.position = position;
    }

    fn update_cursor_position_history(&mut self, index: CursorIndex, last_position: Position) {
        if !self.is_cursor_index_main(index) {
            return;
        }

        let cursor = self.get_cursor_mut(index);

        if (cursor.position.y - last_position.y).abs() < CURSOR_POSITION_HISTORY_THRESHOLD {
            return;
        }

        self.cursor_position_redo_history.clear();
        self.cursor_position_undo_history.push(last_position);
    }

    pub fn move_cursor(&mut self, index: CursorIndex, delta: Position, should_select: bool) {
        self.update_cursor_selection(index, should_select);

        let cursor = self.get_cursor(index);
        let start_position = cursor.position;
        let desired_visual_x = cursor.desired_visual_x;

        let last_position = cursor.position;

        self.get_cursor_mut(index).position =
            self.move_position_with_desired_visual_x(start_position, delta, Some(desired_visual_x));

        self.update_cursor_position_history(index, last_position);

        if delta.x != 0 {
            self.update_cursor_desired_visual_x(index);
        }
    }

    pub fn move_cursors(&mut self, delta: Position, should_select: bool) {
        for index in self.cursor_indices() {
            self.move_cursor(index, delta, should_select);
        }
    }

    pub fn move_cursor_to_next_word(
        &mut self,
        index: CursorIndex,
        delta_x: isize,
        should_select: bool,
    ) {
        let cursor = self.get_cursor(index);
        let destination = self.move_position_to_next_word(cursor.position, delta_x);

        self.jump_cursor(index, destination, should_select);
    }

    pub fn move_cursors_to_next_word(&mut self, delta_x: isize, should_select: bool) {
        for index in self.cursor_indices() {
            self.move_cursor_to_next_word(index, delta_x, should_select);
        }
    }

    pub fn move_cursor_to_next_paragraph(
        &mut self,
        index: CursorIndex,
        delta_y: isize,
        should_select: bool,
    ) {
        let cursor = self.get_cursor(index);
        let destination = self.move_position_to_next_paragraph(cursor.position, delta_y, true);

        self.jump_cursor(index, destination, should_select);
    }

    pub fn move_cursors_to_next_paragraph(&mut self, delta_y: isize, should_select: bool) {
        for index in self.cursor_indices() {
            self.move_cursor_to_next_paragraph(index, delta_y, should_select);
        }
    }

    pub fn jump_cursor(&mut self, index: CursorIndex, position: Position, should_select: bool) {
        self.update_cursor_selection(index, should_select);

        let last_position = self.get_cursor(index).position;

        self.get_cursor_mut(index).position = self.clamp_position(position);

        self.update_cursor_position_history(index, last_position);
        self.update_cursor_desired_visual_x(index);
    }

    pub fn jump_cursors(&mut self, position: Position, should_select: bool) {
        self.clear_extra_cursors(CursorIndex::Main);
        self.jump_cursor(CursorIndex::Main, position, should_select);
    }

    pub fn start_cursor_selection(&mut self, index: CursorIndex) {
        let position = self.get_cursor(index).position;
        self.get_cursor_mut(index).selection_anchor = Some(position);
    }

    pub fn end_cursor_selection(&mut self, index: CursorIndex) {
        self.get_cursor_mut(index).selection_anchor = None;
    }

    pub fn update_cursor_selection(&mut self, index: CursorIndex, should_select: bool) {
        let cursor = self.get_cursor(index);

        if should_select && cursor.selection_anchor.is_none() {
            self.start_cursor_selection(index);
        } else if !should_select && cursor.selection_anchor.is_some() {
            self.end_cursor_selection(index);
        }
    }

    fn get_cursor_visual_x(&self, index: CursorIndex) -> isize {
        let cursor = self.get_cursor(index);

        let leading_text = &self.lines[cursor.position.y as usize][..cursor.position.x as usize];

        Gfx::measure_text(leading_text)
    }

    fn update_cursor_desired_visual_x(&mut self, index: CursorIndex) {
        self.get_cursor_mut(index).desired_visual_x = self.get_cursor_visual_x(index);
    }

    pub fn add_cursor(&mut self, position: Position) {
        let position = self.clamp_position(position);

        self.cursors.push(Cursor::new(position, 0));
        self.update_cursor_desired_visual_x(CursorIndex::Main);
    }

    pub fn unwrap_cursor_index(&self, index: CursorIndex) -> usize {
        let main_cursor_index = self.get_main_cursor_index();

        index.unwrap_or(main_cursor_index)
    }

    pub fn is_cursor_index_main(&self, index: CursorIndex) -> bool {
        match index {
            CursorIndex::Some(index) => index == self.get_main_cursor_index(),
            CursorIndex::Main => true,
        }
    }

    pub fn remove_cursor(&mut self, index: CursorIndex) {
        if self.cursors.len() < 2 {
            return;
        }

        let index = self.unwrap_cursor_index(index);
        self.cursors.remove(index);
    }

    pub fn get_cursor(&self, index: CursorIndex) -> &Cursor {
        let index = self.unwrap_cursor_index(index);
        &self.cursors[index]
    }

    fn get_cursor_mut(&mut self, index: CursorIndex) -> &mut Cursor {
        let index = self.unwrap_cursor_index(index);
        &mut self.cursors[index]
    }

    pub fn set_cursor_selection(&mut self, index: CursorIndex, selection: Option<Selection>) {
        let cursor = self.get_cursor_mut(index);

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

    fn get_main_cursor_index(&self) -> usize {
        self.cursors.len() - 1
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn collect_chars(&self, start: Position, end: Position, buffer: &mut Vec<char>) {
        let start = self.clamp_position(start);
        let end = self.clamp_position(end);

        if start.y == end.y {
            buffer
                .extend_from_slice(&self.lines[start.y as usize][start.x as usize..end.x as usize]);
        } else {
            buffer.extend_from_slice(&self.lines[start.y as usize][start.x as usize..]);
            buffer.push('\n');

            for line in &self.lines[(start.y + 1) as usize..end.y as usize] {
                buffer.extend_from_slice(line);
                buffer.push('\n');
            }

            buffer.extend_from_slice(&self.lines[end.y as usize][..end.x as usize]);
        }
    }

    pub fn undo(&mut self, line_pool: &mut LinePool, action_kind: ActionKind) {
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
                            .resize(index + 1, Cursor::new(Position::zero(), 0));
                    }

                    let mut cursor = Cursor::new(position, 0);
                    cursor.selection_anchor = selection_anchor;

                    self.cursors[index] = cursor;
                    self.update_cursor_desired_visual_x(CursorIndex::Some(index));
                }
                Action::Insert { start, end } => {
                    were_cursors_reset = false;

                    self.delete_as_action_kind(
                        start,
                        end,
                        line_pool,
                        reverse_action_kind,
                        popped_action.time,
                    );
                }
                Action::Delete { start, chars_start } => {
                    were_cursors_reset = false;

                    let mut undo_char_buffer = self.undo_char_buffer.take().unwrap();
                    undo_char_buffer.clear();
                    undo_char_buffer.extend_from_slice(
                        &action_history!(self, action_kind).deleted_chars[chars_start..],
                    );

                    self.insert_as_action_kind(
                        start,
                        &undo_char_buffer,
                        line_pool,
                        reverse_action_kind,
                        popped_action.time,
                    );

                    undo_char_buffer.clear();
                    self.undo_char_buffer = Some(undo_char_buffer);

                    action_history!(self, action_kind)
                        .deleted_chars
                        .truncate(chars_start);
                }
            }
        }
    }

    pub fn add_cursors_to_action_history(&mut self, action_kind: ActionKind, time: f32) {
        if self.kind == DocKind::Output {
            return;
        }

        for index in self.cursor_indices() {
            let Cursor {
                position,
                selection_anchor,
                ..
            } = *self.get_cursor(index);

            let index = self.unwrap_cursor_index(index);

            action_history!(self, action_kind).push_set_cursor(
                index,
                position,
                selection_anchor,
                time,
            );
        }
    }

    fn mark_line_dirty(&mut self, y: isize) {
        self.unhighlighted_line_y = self.unhighlighted_line_y.min(y);
        self.needs_tokenization = true;
    }

    pub fn delete(&mut self, start: Position, end: Position, line_pool: &mut LinePool, time: f32) {
        self.delete_as_action_kind(start, end, line_pool, ActionKind::Done, time);
    }

    pub fn delete_as_action_kind(
        &mut self,
        start: Position,
        end: Position,
        line_pool: &mut LinePool,
        action_kind: ActionKind,
        time: f32,
    ) {
        if action_kind == ActionKind::Done {
            self.redo_history.clear();
        }

        self.mark_line_dirty(start.y);
        self.is_saved = false;
        self.version += 1;

        self.add_cursors_to_action_history(action_kind, time);

        let start = self.clamp_position(start);
        let end = self.clamp_position(end);

        let mut undo_char_buffer = self.undo_char_buffer.take().unwrap();
        undo_char_buffer.clear();

        self.collect_chars(start, end, &mut undo_char_buffer);

        if self.kind != DocKind::Output {
            let deleted_chars_start = action_history!(self, action_kind).deleted_chars.len();

            action_history!(self, action_kind)
                .deleted_chars
                .extend_from_slice(&undo_char_buffer);

            action_history!(self, action_kind).push_delete(start, deleted_chars_start, time);
        }

        self.undo_char_buffer = Some(undo_char_buffer);

        if start.y == end.y {
            self.lines[start.y as usize].drain(start.x as usize..end.x as usize);
        } else {
            let (start_lines, end_lines) = self.lines.split_at_mut(end.y as usize);

            let start_line = &mut start_lines[start.y as usize];
            let end_line = end_lines.first().unwrap();

            start_line.truncate(start.x as usize);
            start_line.extend_from_slice(&end_line[end.x as usize..]);
            line_pool.push(self.lines.remove(end.y as usize));

            for removed_line in self.lines.drain((start.y + 1) as usize..end.y as usize) {
                line_pool.push(removed_line);
            }
        }

        // Shift the cursor:
        for index in self.cursor_indices() {
            let cursor = self.get_cursor_mut(index);

            cursor.position = Self::shift_position_by_delete(start, end, cursor.position);

            if let Some(selection_anchor) = cursor.selection_anchor {
                cursor.selection_anchor =
                    Some(Self::shift_position_by_delete(start, end, selection_anchor));
            }

            self.update_cursor_desired_visual_x(index);
        }
    }

    fn shift_position_by_delete(start: Position, end: Position, position: Position) -> Position {
        let influence_end = end.min(position);

        if influence_end <= start {
            return position;
        }

        if influence_end.y == position.y && influence_end.x <= position.x {
            Position::new(
                position.x - (influence_end.x - start.x),
                position.y - (influence_end.y - start.y),
            )
        } else if influence_end.y < position.y {
            Position::new(position.x, position.y - (influence_end.y - start.y))
        } else {
            position
        }
    }

    pub fn insert(
        &mut self,
        start: Position,
        text: impl IntoIterator<Item = impl Borrow<char>>,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        self.insert_as_action_kind(start, text, line_pool, ActionKind::Done, time);
    }

    pub fn insert_as_action_kind(
        &mut self,
        start: Position,
        text: impl IntoIterator<Item = impl Borrow<char>>,
        line_pool: &mut LinePool,
        action_kind: ActionKind,
        time: f32,
    ) {
        if action_kind == ActionKind::Done {
            self.redo_history.clear();
        }

        self.mark_line_dirty(start.y);
        self.is_saved = false;
        self.version += 1;

        self.add_cursors_to_action_history(action_kind, time);

        let start = self.clamp_position(start);
        let mut position = self.clamp_position(start);

        for c in text {
            let c = *c.borrow();

            match c {
                '\r' => continue,
                '\n' => {
                    if self.kind == DocKind::SingleLine {
                        continue;
                    }

                    let new_y = position.y as usize + 1;
                    let split_x = position.x as usize;

                    position.y += 1;
                    position.x = 0;

                    self.lines.insert(new_y, line_pool.pop());

                    let (old, new) = self.lines.split_at_mut(new_y);

                    let old = old.last_mut().unwrap();
                    let new = new.first_mut().unwrap();

                    new.extend_from_slice(&old[split_x..]);
                    old.truncate(split_x);

                    continue;
                }
                _ => {}
            }

            self.lines[position.y as usize].insert(position.x as usize, c);
            position.x += 1;
        }

        if self.kind != DocKind::Output {
            action_history!(self, action_kind).push_insert(start, position, time);
        }

        // Shift the cursor:
        for index in self.cursor_indices() {
            let cursor = self.get_cursor_mut(index);

            cursor.position = Self::shift_position_by_insert(start, position, cursor.position);

            if let Some(selection_anchor) = cursor.selection_anchor {
                cursor.selection_anchor = Some(Self::shift_position_by_insert(
                    start,
                    position,
                    selection_anchor,
                ));
            }

            self.update_cursor_desired_visual_x(index);
        }
    }

    fn shift_position_by_insert(start: Position, end: Position, position: Position) -> Position {
        if start.y == position.y && start.x <= position.x {
            Position::new(position.x + end.x - start.x, position.y + end.y - start.y)
        } else if start.y < position.y {
            Position::new(position.x, position.y + end.y - start.y)
        } else {
            position
        }
    }

    pub fn insert_at_cursors(&mut self, text: &[char], line_pool: &mut LinePool, time: f32) {
        for index in self.cursor_indices() {
            self.insert_at_cursor(index, text, line_pool, time);
        }
    }

    pub fn insert_at_cursor(
        &mut self,
        index: CursorIndex,
        text: &[char],
        line_pool: &mut LinePool,
        time: f32,
    ) {
        if let Some(selection) = self.get_cursor(index).get_selection() {
            self.delete(selection.start, selection.end, line_pool, time);
            self.end_cursor_selection(index);
        }

        let start = self.get_cursor(index).position;
        self.insert(start, text, line_pool, time);
    }

    pub fn search(&self, text: &[char], start: Position, is_reverse: bool) -> Option<Position> {
        let start = self.clamp_position(start);
        let start = Position::new(start.x.min(self.get_line_len(start.y) - 1).max(0), start.y);

        let step = if is_reverse { -1 } else { 1 };

        let text_start_index = if is_reverse {
            text.len() as isize - 1
        } else {
            0
        };

        let mut position = start;
        let mut match_index = 0;

        if is_reverse {
            position.x += step;
        }

        loop {
            let line = &self.lines[position.y as usize];
            let last_position = position;
            let status;
            (position, status) = self.step_wrapped(line, start, position, step);

            match status {
                StepStatus::None => {}
                StepStatus::Wrapped => {
                    match_index = 0;
                    continue;
                }
                StepStatus::Done => break,
            }

            if text[(match_index as isize * step + text_start_index) as usize]
                == line[position.x as usize]
            {
                match_index += 1;

                if match_index >= text.len() {
                    return Some(Position::new(
                        position.x + 1 - text.len() as isize + text_start_index,
                        position.y,
                    ));
                }
            } else {
                if match_index > 0 {
                    position = last_position;
                }

                match_index = 0;
            }
        }

        None
    }

    // Positions returned by this function are in bounds as long as the status is None.
    fn step_wrapped(
        &self,
        line: &Line,
        start: Position,
        position: Position,
        step: isize,
    ) -> (Position, StepStatus) {
        let mut x = position.x;
        let mut y = position.y;
        let mut status = StepStatus::None;

        x = x.min(line.len() as isize);
        x += step;

        if x == start.x && y == start.y {
            return (position, StepStatus::Done);
        }

        if x < 0 {
            x = isize::MAX;
            y -= 1;
            status = StepStatus::Wrapped;
        } else if x >= line.len() as isize {
            x = -1;
            y += 1;
            status = StepStatus::Wrapped;
        };

        y = y.rem_euclid(self.lines.len() as isize);

        (Position::new(x, y), status)
    }

    pub fn end(&self) -> Position {
        let mut position = Position::new(0, self.lines().len() as isize - 1);
        position.x = self.get_line_len(position.y);

        position
    }

    pub fn get_char(&self, position: Position) -> char {
        let position = self.clamp_position(position);
        let line = &self.lines[position.y as usize];

        if position.x == line.len() as isize {
            '\n'
        } else {
            line[position.x as usize]
        }
    }

    // It's ok for the x position to equal the length of the line.
    // That represents the cursor being right before the newline sequence.
    fn clamp_position(&self, position: Position) -> Position {
        let max_y = self.lines.len() as isize - 1;
        let clamped_y = position.y.clamp(0, max_y);

        let max_x = self.lines[clamped_y as usize].len() as isize;
        let clamped_x = position.x.clamp(0, max_x);

        Position::new(clamped_x, clamped_y)
    }

    pub fn position_to_visual(
        &self,
        position: Position,
        camera_position: VisualPosition,
        gfx: &Gfx,
    ) -> VisualPosition {
        let position = self.clamp_position(position);
        let leading_text = &self.lines[position.y as usize][..position.x as usize];

        let visual_x = Gfx::measure_text(leading_text);

        VisualPosition::new(
            visual_x as f32 * gfx.glyph_width() - camera_position.x,
            position.y as f32 * gfx.line_height() - camera_position.y,
        )
    }

    pub fn visual_to_position(
        &self,
        visual: VisualPosition,
        camera_position: VisualPosition,
        gfx: &Gfx,
    ) -> Position {
        let mut position = Position::new(
            ((visual.x + camera_position.x) / gfx.glyph_width()) as isize,
            ((visual.y + camera_position.y) / gfx.line_height()) as isize,
        );

        let desired_x = position.x;
        position = self.clamp_position(position);
        position.x = Gfx::find_x_for_visual_x(&self.lines[position.y as usize], desired_x);

        position
    }

    pub fn trim_trailing_whitespace(&mut self, line_pool: &mut LinePool, time: f32) {
        for y in 0..self.lines.len() {
            let line = &self.lines[y];
            let mut whitespace_start = 0;

            for (i, c) in line.iter().enumerate().rev() {
                if !c.is_whitespace() {
                    whitespace_start = i + 1;
                    break;
                }
            }

            if whitespace_start < line.len() {
                let start = Position::new(whitespace_start as isize, y as isize);
                let end = Position::new(line.len() as isize, y as isize);

                self.delete(start, end, line_pool, time);
            }
        }
    }

    fn set_path(&mut self, path: PathBuf) -> io::Result<()> {
        self.path = Some(if path.is_absolute() {
            path
        } else {
            absolute(path)?
        });

        Ok(())
    }

    pub fn save(&mut self, path: PathBuf) -> io::Result<()> {
        let string = self.to_string();

        File::create(&path)?.write_all(string.as_bytes())?;

        self.set_path(path)?;
        self.is_saved = true;
        self.expected_change_count = EXPECTED_CHANGE_COUNT_ON_SAVE;

        Ok(())
    }

    pub fn is_change_unexpected(&mut self) -> bool {
        if self.expected_change_count == 0 {
            true
        } else {
            self.expected_change_count -= 1;

            false
        }
    }

    fn reset_cursors(&mut self) {
        self.cursors.clear();
        self.cursors.push(Cursor::new(Position::zero(), 0));
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

    pub fn drain(&mut self, line_pool: &mut LinePool) -> Drain<Line> {
        self.line_ending = LineEnding::default();

        self.mark_line_dirty(0);
        self.reset_edit_state();

        self.lines.push(line_pool.pop());

        self.lines.drain(..self.lines.len() - 1)
    }

    pub fn clear(&mut self, line_pool: &mut LinePool) {
        for line in self.drain(line_pool) {
            line_pool.push(line);
        }
    }

    pub fn load(&mut self, path: PathBuf, line_pool: &mut LinePool, time: f32) -> io::Result<()> {
        self.clear(line_pool);

        let string = read_to_string(&path)?;

        let (line_ending, len) = self.get_line_ending_and_len(&string);

        self.insert(Position::zero(), string.chars().take(len), line_pool, time);
        self.reset_edit_state();
        self.line_ending = line_ending;

        self.set_path(path)?;

        Ok(())
    }

    pub fn reload(&mut self, buffers: &mut EditorBuffers, time: f32) -> io::Result<()> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };

        let string = read_to_string(path)?;

        let cursor_buffer = buffers.cursors.get_mut();

        self.backup_cursors(cursor_buffer);

        self.delete(Position::zero(), self.end(), &mut buffers.lines, time);

        let (line_ending, len) = self.get_line_ending_and_len(&string);

        self.line_ending = line_ending;
        self.insert(
            Position::zero(),
            string.chars().take(len),
            &mut buffers.lines,
            time,
        );

        self.is_saved = true;

        self.restore_cursors(cursor_buffer);

        Ok(())
    }

    pub fn backup_cursors(&self, buffer: &mut Vec<Cursor>) {
        buffer.clear();
        buffer.extend_from_slice(&self.cursors);
    }

    pub fn restore_cursors(&mut self, buffer: &[Cursor]) {
        for (index, backup) in self.cursor_indices().zip(buffer) {
            let position = self.clamp_position(backup.position);
            let selection_anchor = backup
                .selection_anchor
                .map(|selection_anchor| self.clamp_position(selection_anchor));

            *self.get_cursor_mut(index) = Cursor {
                position,
                selection_anchor,
                desired_visual_x: backup.desired_visual_x,
            };
        }
    }

    fn get_line_ending_and_len(&self, string: &str) -> (LineEnding, usize) {
        let mut len = 0;
        let mut line_ending = LineEnding::default();
        let mut last_char_was_cr = false;

        for c in string.chars() {
            match c {
                '\r' => {
                    last_char_was_cr = true;
                    line_ending = LineEnding::CrLf;
                }
                '\n' => {
                    if !last_char_was_cr {
                        line_ending = LineEnding::Lf;
                    }

                    last_char_was_cr = false;

                    if self.kind == DocKind::SingleLine {
                        break;
                    }
                }
                _ => {
                    last_char_was_cr = false;
                }
            }

            len += 1;
        }

        (line_ending, len)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn file_name(&self) -> &str {
        const DEFAULT_NAME: &str = "Unnamed";

        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .or(self.display_name)
            .unwrap_or(DEFAULT_NAME)
    }

    pub fn is_saved(&self) -> bool {
        self.is_saved || self.kind == DocKind::Output
    }

    pub fn is_worthless(&self) -> bool {
        self.path.is_none() && self.is_saved()
    }

    pub fn has_selection(&self) -> bool {
        for index in self.cursor_indices() {
            if self.get_cursor(index).get_selection().is_some() {
                return true;
            }
        }

        false
    }

    pub fn kind(&self) -> DocKind {
        self.kind
    }

    pub fn copy_at_cursors(&mut self, text: &mut Vec<char>) -> bool {
        let mut was_copy_implicit = true;

        for index in self.cursor_indices() {
            let cursor = self.get_cursor(index);

            if let Some(selection) = cursor.get_selection() {
                was_copy_implicit = false;

                self.collect_chars(selection.start, selection.end, text);
            } else {
                self.copy_line_at_position(cursor.position, text);
            }

            if self.unwrap_cursor_index(index) != self.cursors_len() - 1 {
                text.push('\n');
            }
        }

        was_copy_implicit
    }

    pub fn copy_line_at_position(&mut self, position: Position, text: &mut Vec<char>) {
        let start = Position::new(0, position.y);
        let end = Position::new(self.get_line_len(start.y), start.y);

        self.collect_chars(start, end, text);
        text.push('\n');
    }

    pub fn paste_at_cursor(
        &mut self,
        index: CursorIndex,
        text: &[char],
        was_copy_implicit: bool,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let mut start = self.get_cursor(index).position;

        if let Some(selection) = self.get_cursor(index).get_selection() {
            self.delete(selection.start, selection.end, line_pool, time);
            self.end_cursor_selection(index);
        } else if was_copy_implicit {
            start.x = 0;
        }

        self.insert(start, text, line_pool, time);
    }

    pub fn paste_at_cursors(
        &mut self,
        text: &[char],
        was_copy_implicit: bool,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let mut line_count = 1;

        for c in text {
            if *c == '\n' {
                line_count += 1;
            }
        }

        let do_spread_lines_between_cursors =
            self.cursors_len() > 1 && line_count % self.cursors_len() == 0;

        if do_spread_lines_between_cursors {
            let lines_per_cursor = line_count / self.cursors_len();
            let mut i = 0;

            for index in self.cursor_indices() {
                for line_i in 0..lines_per_cursor {
                    while i < text.len() {
                        let c = text[i];
                        i += 1;

                        if c == '\n' {
                            if line_i < lines_per_cursor - 1 {
                                self.paste_at_cursor(
                                    index,
                                    &[c],
                                    was_copy_implicit,
                                    line_pool,
                                    time,
                                );
                            }

                            break;
                        }

                        self.paste_at_cursor(index, &[c], was_copy_implicit, line_pool, time);
                    }
                }
            }
        } else {
            for index in self.cursor_indices() {
                self.paste_at_cursor(index, text, was_copy_implicit, line_pool, time);
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
        gfx: &Gfx,
    ) {
        let end = self.visual_to_position(
            VisualPosition::new(0.0, camera_position.y + bounds.height),
            camera_position,
            gfx,
        );

        self.syntax_highlighter.update(
            &self.lines,
            syntax,
            self.unhighlighted_line_y as usize,
            end.y as usize,
        );

        self.unhighlighted_line_y = end.y + 1;
    }

    pub fn recycle_highlighted_lines_up_to_y(&mut self, y: usize) {
        self.syntax_highlighter.recycle_highlighted_lines_up_to_y(y);
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
            let cursor = self.get_cursor(index);
            let position = cursor.position;
            let selection = cursor.get_selection();

            for other_index in self.cursor_indices() {
                if self.unwrap_cursor_index(index) == self.unwrap_cursor_index(other_index) {
                    continue;
                }

                let other_cursor = self.get_cursor(other_index);

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

    pub fn add_cursor_at_next_occurance(&mut self) {
        let cursor = self.get_cursor(CursorIndex::Main);

        let Some(selection) = cursor.get_selection() else {
            self.select_current_word_at_cursors();
            return;
        };

        if selection.start.y != selection.end.y {
            return;
        }

        let Some(line) = self.get_line(cursor.position.y) else {
            return;
        };

        let Some(position) = self.search(
            &line[selection.start.x as usize..selection.end.x as usize],
            cursor.position,
            false,
        ) else {
            return;
        };

        self.add_cursor(position);

        let end = self.move_position(
            position,
            Position::new(selection.end.x - selection.start.x, 0),
        );

        self.jump_cursor(CursorIndex::Main, end, true);
    }

    pub fn select_current_line_at_position(&self, position: Position) -> Selection {
        let mut start = Position::new(0, position.y);
        let mut end = Position::new(self.get_line_len(start.y), start.y);

        if start.y as usize == self.lines().len() - 1 {
            start = self.move_position(start, Position::new(-1, 0));
        } else {
            end = self.move_position(end, Position::new(1, 0));
        }

        Selection { start, end }
    }

    pub fn select_current_line_at_cursors(&mut self) {
        for index in self.cursor_indices() {
            let position = self.get_cursor(index).position;
            let selection = self.select_current_line_at_position(position);

            let cursor = self.get_cursor_mut(index);

            cursor.selection_anchor = Some(selection.start);
            cursor.position = selection.end;
        }
    }

    pub fn select_current_word_at_position(&self, mut position: Position) -> Selection {
        let line_len = self.get_line_len(position.y);

        if position.x < line_len {
            position = self.move_position(position, Position::new(1, 0));
        }

        if position.x > 0 {
            position = self.move_position_to_next_word(position, -1);
        }

        let start = position;

        if position.x < line_len {
            position = self.move_position_to_next_word(position, 1);
        }

        let end = position;

        Selection { start, end }
    }

    pub fn select_current_word_at_cursors(&mut self) {
        for index in self.cursor_indices() {
            let position = self.get_cursor(index).position;
            let selection = self.select_current_word_at_position(position);

            let cursor = self.get_cursor_mut(index);

            cursor.selection_anchor = Some(selection.start);
            cursor.position = selection.end;
        }
    }

    pub fn version(&self) -> usize {
        self.version
    }
}

impl Display for Doc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let line_ending_chars: &[char] = match self.line_ending {
            LineEnding::Lf => &['\n'],
            LineEnding::CrLf => &['\r', '\n'],
        };

        for (i, line) in self.lines.iter().enumerate() {
            for c in line {
                f.write_char(*c)?;
            }

            if i != self.lines.len() - 1 {
                for c in line_ending_chars {
                    f.write_char(*c)?;
                }
            }
        }

        Ok(())
    }
}
