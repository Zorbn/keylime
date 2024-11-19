use std::{
    fmt::{Display, Write as _},
    fs::{read_to_string, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use crate::{
    action_history::{Action, ActionHistory, ActionKind},
    char_category::CharCategory,
    cursor::Cursor,
    cursor_index::{CursorIndex, CursorIndices},
    gfx::Gfx,
    line_pool::{Line, LinePool},
    position::Position,
    selection::Selection,
    syntax_highlighter::{HighlightedLine, Syntax, SyntaxHighlighter},
    visual_position::VisualPosition,
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

#[derive(PartialEq, Eq)]
pub enum DocKind {
    MultiLine,
    SingleLine,
}

pub struct Doc {
    path: Option<PathBuf>,
    is_saved: bool,
    version: usize,

    lines: Vec<Line>,
    cursors: Vec<Cursor>,
    line_ending: LineEnding,

    undo_history: ActionHistory,
    redo_history: ActionHistory,
    undo_char_buffer: Option<Vec<char>>,

    syntax_highlighter: SyntaxHighlighter,
    unhighlighted_line_y: isize,

    kind: DocKind,
}

impl Doc {
    pub fn new(line_pool: &mut LinePool, kind: DocKind) -> Self {
        let lines = vec![line_pool.pop()];

        let mut doc = Self {
            path: None,
            is_saved: true,
            version: 0,

            lines,
            cursors: Vec::new(),
            line_ending: LineEnding::default(),

            undo_history: ActionHistory::new(),
            redo_history: ActionHistory::new(),
            undo_char_buffer: Some(Vec::new()),

            syntax_highlighter: SyntaxHighlighter::new(),
            unhighlighted_line_y: 0,

            kind,
        };

        doc.reset_cursors();

        doc
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
                new_x = Gfx::find_x_for_visual_x(
                    self.lines[new_y as usize].iter().copied(),
                    desired_visual_x,
                );
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

    pub fn move_cursor(&mut self, index: CursorIndex, delta: Position, should_select: bool) {
        self.update_cursor_selection(index, should_select);

        let cursor = self.get_cursor(index);
        let start_position = cursor.position;
        let desired_visual_x = cursor.desired_visual_x;

        self.get_cursor_mut(index).position =
            self.move_position_with_desired_visual_x(start_position, delta, Some(desired_visual_x));

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

        self.get_cursor_mut(index).position = self.clamp_position(position);
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
        let visual_x = Gfx::measure_text(leading_text.iter().copied());

        visual_x
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

    pub fn get_cursor_mut(&mut self, index: CursorIndex) -> &mut Cursor {
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

    fn mark_line_unhighlighted(&mut self, y: isize) {
        self.unhighlighted_line_y = self.unhighlighted_line_y.min(y);
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

        self.mark_line_unhighlighted(start.y);
        self.is_saved = false;
        self.version += 1;

        self.add_cursors_to_action_history(action_kind, time);

        let start = self.clamp_position(start);
        let end = self.clamp_position(end);

        let mut undo_char_buffer = self.undo_char_buffer.take().unwrap();
        undo_char_buffer.clear();

        self.collect_chars(start, end, &mut undo_char_buffer);

        let deleted_chars_start = action_history!(self, action_kind).deleted_chars.len();

        action_history!(self, action_kind)
            .deleted_chars
            .extend_from_slice(&undo_char_buffer);

        self.undo_char_buffer = Some(undo_char_buffer);

        action_history!(self, action_kind).push_delete(start, deleted_chars_start, time);

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
            let cursor = self.get_cursor(index);

            let cursor_effect_end = end.min(cursor.position);

            if cursor_effect_end <= start {
                continue;
            }

            if cursor_effect_end.y == cursor.position.y && cursor_effect_end.x <= cursor.position.x
            {
                let cursor = self.get_cursor_mut(index);

                cursor.position.x -= cursor_effect_end.x - start.x;
                cursor.position.y -= cursor_effect_end.y - start.y;

                self.update_cursor_desired_visual_x(index);
            } else if cursor_effect_end.y < cursor.position.y {
                let cursor = self.get_cursor_mut(index);

                cursor.position.y -= cursor_effect_end.y - start.y;
            }
        }
    }

    pub fn insert(&mut self, start: Position, text: &[char], line_pool: &mut LinePool, time: f32) {
        self.insert_as_action_kind(start, text, line_pool, ActionKind::Done, time);
    }

    pub fn insert_as_action_kind(
        &mut self,
        start: Position,
        text: &[char],
        line_pool: &mut LinePool,
        action_kind: ActionKind,
        time: f32,
    ) {
        if action_kind == ActionKind::Done {
            self.redo_history.clear();
        }

        self.mark_line_unhighlighted(start.y);
        self.is_saved = false;
        self.version += 1;

        self.add_cursors_to_action_history(action_kind, time);

        let start = self.clamp_position(start);
        let mut position = self.clamp_position(start);

        for c in text {
            if *c == '\n' {
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

            self.lines[position.y as usize].insert(position.x as usize, *c);
            position.x += 1;
        }

        action_history!(self, action_kind).push_insert(start, position, time);

        // Shift the cursor:
        for index in self.cursor_indices() {
            let cursor = self.get_cursor(index);

            if start.y == cursor.position.y && start.x <= cursor.position.x {
                let cursor = self.get_cursor_mut(index);

                cursor.position.x += position.x - start.x;
                cursor.position.y += position.y - start.y;

                self.update_cursor_desired_visual_x(index);
            } else if start.y < cursor.position.y {
                let cursor = self.get_cursor_mut(index);

                cursor.position.y += position.y - start.y;
            }
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

    pub fn search(&self, text: &[char], start: Position, do_wrap: bool) -> Option<Position> {
        let start = self.clamp_position(start);

        let mut x = start.x as usize;

        for y in start.y..self.lines.len() as isize {
            let line = &self.lines[y as usize];

            let search_len = line.len().saturating_sub(text.len().saturating_sub(1));

            while x < search_len {
                let start_x = x as isize;
                let mut found_text = true;

                for c in text {
                    if *c != line[x] {
                        found_text = false;
                        break;
                    }

                    x += 1;
                }

                if found_text {
                    return Some(Position::new(start_x, y));
                }

                x += 1;
            }

            x = 0;
        }

        if do_wrap {
            self.search(text, Position::zero(), false)
        } else {
            None
        }
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

    pub fn insert_at_cursors(&mut self, text: &[char], line_pool: &mut LinePool, time: f32) {
        for index in self.cursor_indices() {
            self.insert_at_cursor(index, text, line_pool, time);
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
        camera_y: f32,
        gfx: &Gfx,
    ) -> VisualPosition {
        let position = self.clamp_position(position);
        let leading_text = &self.lines[position.y as usize][..position.x as usize];

        let visual_x = Gfx::measure_text(leading_text.iter().copied());

        VisualPosition::new(
            visual_x as f32 * gfx.glyph_width(),
            position.y as f32 * gfx.line_height() - camera_y,
        )
    }

    pub fn visual_to_position(&self, visual: VisualPosition, camera_y: f32, gfx: &Gfx) -> Position {
        let mut position = Position::new(
            (visual.x / gfx.glyph_width()) as isize,
            ((visual.y + camera_y) / gfx.line_height()) as isize,
        );

        let desired_x = position.x;
        position = self.clamp_position(position);
        position.x =
            Gfx::find_x_for_visual_x(self.lines[position.y as usize].iter().copied(), desired_x);

        position
    }

    pub fn save(&mut self, path: PathBuf) -> io::Result<()> {
        let string = self.to_string();

        File::create(&path)?.write_all(string.as_bytes())?;

        self.path = Some(path);
        self.is_saved = true;

        Ok(())
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

    pub fn clear(&mut self, line_pool: &mut LinePool) {
        self.undo_history.clear();
        self.redo_history.clear();

        self.reset_cursors();
        self.line_ending = LineEnding::default();

        self.unhighlighted_line_y = 0;
        self.is_saved = true;
        self.version = 0;

        for line in self.lines.drain(..) {
            line_pool.push(line);
        }

        self.lines.push(line_pool.pop());
    }

    pub fn load(&mut self, path: &Path, line_pool: &mut LinePool) -> io::Result<()> {
        self.clear(line_pool);

        for line in self.lines.drain(..) {
            line_pool.push(line);
        }

        let string = read_to_string(path)?;

        let mut current_line = line_pool.pop();
        let mut last_char_was_cr = false;

        for c in string.chars() {
            match c {
                '\r' => {
                    last_char_was_cr = true;
                    self.line_ending = LineEnding::CrLf;
                }
                '\n' => {
                    if !last_char_was_cr {
                        self.line_ending = LineEnding::Lf;
                    }

                    last_char_was_cr = false;

                    if self.kind == DocKind::SingleLine {
                        break;
                    }

                    self.lines.push(current_line);
                    current_line = line_pool.pop();
                }
                _ => {
                    last_char_was_cr = false;
                    current_line.push(c);
                }
            }
        }

        self.lines.push(current_line);

        self.path = Some(path.to_path_buf());

        Ok(())
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
            .unwrap_or(DEFAULT_NAME)
    }

    pub fn is_saved(&self) -> bool {
        self.is_saved
    }

    pub fn copy_at_cursors(&mut self, text: &mut Vec<char>) -> bool {
        let mut was_copy_implicit = true;

        for index in self.cursor_indices() {
            let cursor = self.get_cursor(index);

            if let Some(selection) = cursor.get_selection() {
                was_copy_implicit = false;

                self.collect_chars(selection.start, selection.end, text);
            } else {
                let start = Position::new(0, cursor.position.y);
                let end = Position::new(self.get_line_len(start.y), start.y);

                self.collect_chars(start, end, text);
                text.push('\n');
            }

            if self.unwrap_cursor_index(index) != self.cursors_len() - 1 {
                text.push('\n');
            }
        }

        was_copy_implicit
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

    pub fn update_highlights(&mut self, camera_y: f32, gfx: &Gfx, syntax: &Syntax) {
        let end = self.visual_to_position(
            VisualPosition::new(0.0, camera_y + gfx.height()),
            camera_y,
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

    pub fn highlighted_lines(&self) -> &[HighlightedLine] {
        self.syntax_highlighter.highlighted_lines()
    }

    pub fn combine_overlapping_cursors(&mut self) {
        for index in self.cursor_indices().rev() {
            let cursor = self.get_cursor(index);
            let position = cursor.position;
            let selection = cursor.get_selection();

            for other_index in self.cursor_indices() {
                if index == other_index {
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
