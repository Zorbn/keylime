use std::{fs::File, io::{self, Read, Write}};

use crate::{
    cursor::Cursor,
    gfx::Gfx,
    line_pool::{Line, LinePool},
    position::Position,
    visual_position::VisualPosition,
};

enum LineEnding {
    Lf,
    CrLf,
}

pub struct Doc {
    lines: Vec<Line>,
    cursor: Cursor,
    line_ending: LineEnding,
}

impl Doc {
    pub fn new(line_pool: &mut LinePool) -> Self {
        let lines = vec![line_pool.pop()];

        Self {
            lines,
            cursor: Cursor::new(Position::zero(), 0),
            line_ending: LineEnding::CrLf,
        }
    }

    pub fn move_position(&self, position: Position, delta: Position) -> Position {
        self.move_position_with_desired_visual_x(position, delta, None)
    }

    pub fn move_position_with_desired_visual_x(&self, position: Position, delta: Position, desired_visual_x: Option<isize>) -> Position {
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
                new_x = Gfx::find_x_for_visual_x(self.lines[new_y as usize].iter().copied(), desired_visual_x);
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

    pub fn move_cursor(&mut self, delta: Position, should_select: bool) {
        self.update_cursor_selection(should_select);

        self.cursor.position = self.move_position_with_desired_visual_x(self.cursor.position, delta, Some(self.cursor.desired_visual_x));

        if delta.x != 0 {
            self.update_cursor_desired_visual_x();
        }
    }

    pub fn jump_cursor(&mut self, position: Position, should_select: bool) {
        self.update_cursor_selection(should_select);

        self.cursor.position = self.clamp_position(position);
        self.update_cursor_desired_visual_x();
    }

    pub fn start_cursor_selection(&mut self) {
        self.cursor.selection_anchor = Some(self.cursor.position);
    }

    pub fn end_cursor_selection(&mut self) {
        self.cursor.selection_anchor = None;
    }

    pub fn update_cursor_selection(&mut self, should_select: bool) {
        if should_select && self.cursor.selection_anchor.is_none() {
            self.start_cursor_selection();
        } else if !should_select && self.cursor.selection_anchor.is_some() {
            self.end_cursor_selection();
        }
    }

    fn get_cursor_visual_x(&self) -> isize {
        let leading_text = &self.lines[self.cursor.position.y as usize][..self.cursor.position.x as usize];
        let visual_x = Gfx::measure_text(leading_text.iter().copied());

        visual_x
    }

    fn update_cursor_desired_visual_x(&mut self) {
        self.cursor.desired_visual_x = self.get_cursor_visual_x();
    }

    pub fn get_cursor(&self) -> &Cursor {
        &self.cursor
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

            let start_line = start_lines.last_mut().unwrap();
            let end_line = end_lines.first().unwrap();

            start_line.truncate(start.x as usize);
            start_line.extend_from_slice(&end_line[end.x as usize..]);

            line_pool.push(self.lines.remove(end.y as usize));

            for removed_line in self.lines.drain((start.y + 1) as usize..end.y as usize) {
                line_pool.push(removed_line);
            }
        }

        // Shift the cursor:
        let cursor_effect_end = if end.y > self.cursor.position.y
            || (end.y == self.cursor.position.y && end.x > self.cursor.position.x)
        {
            self.cursor.position
        } else {
            end
        };

        if cursor_effect_end.y == self.cursor.position.y
            && cursor_effect_end.x <= self.cursor.position.x
        {
            self.cursor.position.x -= cursor_effect_end.x - start.x;
            self.cursor.position.y -= cursor_effect_end.y - start.y;
            self.update_cursor_desired_visual_x();
        } else if cursor_effect_end.y < self.cursor.position.y {
            self.cursor.position.y -= cursor_effect_end.y - start.y;
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
        if start.y == self.cursor.position.y && start.x <= self.cursor.position.x {
            self.cursor.position.x += position.x - start.x;
            self.cursor.position.y += position.y - start.y;
            self.update_cursor_desired_visual_x();
        } else if start.y < self.cursor.position.y {
            self.cursor.position.y += position.y - start.y;
        }
    }

    pub fn insert_at_cursor(&mut self, text: &[char], line_pool: &mut LinePool) {
        if let Some(selection) = self.cursor.get_selection() {
            self.end_cursor_selection();
            self.delete(selection.start, selection.end, line_pool);
        }

        let start = self.get_cursor().position;
        self.insert(start, text, line_pool);
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

    pub fn position_to_visual(&self, position: Position, gfx: &Gfx) -> VisualPosition {
        let position = self.clamp_position(position);
        let leading_text = &self.lines[position.y as usize][..position.x as usize];

        let visual_x = Gfx::measure_text(leading_text.iter().copied());

        VisualPosition::new(
            visual_x as f32 * gfx.glyph_width(),
            position.y as f32 * gfx.line_height(),
        )
    }

    pub fn visual_to_position(&self, visual: VisualPosition, gfx: &Gfx) -> Position {
        let mut position = Position::new(
            (visual.x / gfx.glyph_width()) as isize,
            (visual.y / gfx.line_height()) as isize,
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

    pub fn load(&mut self, file: &mut File, line_pool: &mut LinePool) -> io::Result<usize> {
        self.cursor = Cursor::new(Position::zero(), 0);
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