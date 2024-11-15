use std::{
    fs::File,
    io::{self, Read, Write},
};

use crate::{
    cursor::Cursor,
    cursor_index::{CursorIndex, CursorIndices},
    gfx::Gfx,
    line_pool::{Line, LinePool},
    position::Position,
    selection::Selection,
    visual_position::VisualPosition,
};

enum LineEnding {
    Lf,
    CrLf,
}

pub struct Doc {
    lines: Vec<Line>,
    cursors: Vec<Cursor>,
    line_ending: LineEnding,
}

impl Doc {
    pub fn new(line_pool: &mut LinePool) -> Self {
        let lines = vec![line_pool.pop()];

        let mut doc = Self {
            lines,
            cursors: Vec::new(),
            line_ending: LineEnding::CrLf,
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

    pub fn get_line_len(&self, y: isize) -> isize {
        if y < 0 || y >= self.lines.len() as isize {
            return 0;
        }

        self.lines[y as usize].len() as isize
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

    fn unwrap_cursor_index(&self, index: CursorIndex) -> usize {
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

    fn get_main_cursor_index(&self) -> usize {
        self.cursors.len() - 1
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn delete(&mut self, start: Position, end: Position, line_pool: &mut LinePool) {
        let start = self.clamp_position(start);
        let end = self.clamp_position(end);

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

    pub fn insert(&mut self, start: Position, text: &[char], line_pool: &mut LinePool) {
        let start = self.clamp_position(start);
        let mut position = self.clamp_position(start);

        for i in 0..text.len() {
            if text[i] == '\n' {
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

            self.lines[position.y as usize].insert(position.x as usize, text[i]);
            position.x += 1;
        }

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
    ) {
        if let Some(selection) = self.get_cursor(index).get_selection() {
            self.end_cursor_selection(index);
            self.delete(selection.start, selection.end, line_pool);
        }

        let start = self.get_cursor(index).position;
        self.insert(start, text, line_pool);
    }

    pub fn get_char(&self, position: Position) -> char {
        let position = self.clamp_position(position);
        let line = &self.lines[position.y as usize];

        if position.x >= line.len() as isize {
            '\n'
        } else {
            line[position.x as usize]
        }
    }

    pub fn insert_at_cursors(&mut self, text: &[char], line_pool: &mut LinePool) {
        for index in self.cursor_indices() {
            self.insert_at_cursor(index, text, line_pool);
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

    pub fn save(&mut self, file: &mut File) -> io::Result<usize> {
        let string = self.to_string();

        file.write(string.as_bytes())
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

    pub fn load(&mut self, file: &mut File, line_pool: &mut LinePool) -> io::Result<usize> {
        self.reset_cursors();
        self.line_ending = LineEnding::Lf;

        let mut string = String::new();
        let read = file.read_to_string(&mut string)?;

        for line in self.lines.drain(..) {
            line_pool.push(line);
        }

        let mut current_line = line_pool.pop();

        for c in string.chars() {
            match c {
                '\r' => {
                    self.line_ending = LineEnding::CrLf;
                }
                '\n' => {
                    self.lines.push(current_line);
                    current_line = line_pool.pop();
                }
                _ => {
                    current_line.push(c);
                }
            }
        }

        self.lines.push(current_line);

        Ok(read)
    }
}

impl ToString for Doc {
    fn to_string(&self) -> String {
        let mut string = String::new();

        let line_ending_chars: &[char] = match self.line_ending {
            LineEnding::Lf => &['\n'],
            LineEnding::CrLf => &['\r', '\n'],
        };

        for (i, line) in self.lines.iter().enumerate() {
            string.extend(line);

            if i != self.lines.len() - 1 {
                string.extend(line_ending_chars);
            }
        }

        string
    }
}
