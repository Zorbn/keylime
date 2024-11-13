use crate::{
    cursor::Cursor,
    gfx::Gfx,
    line_pool::{Line, LinePool},
    position::Position,
    visual_position::VisualPosition,
};

pub struct Doc {
    lines: Vec<Line>,
    cursor: Cursor,
}

impl Doc {
    pub fn new(line_pool: &mut LinePool) -> Self {
        let lines = vec![line_pool.pop()];

        Self {
            lines,
            cursor: Cursor::new(Position::zero()),
        }
    }

    pub fn move_position(&self, position: Position, delta: Position) -> Position {
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

            if new_x > self.get_line_len(new_y) {
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

    pub fn move_cursor(&mut self, delta: Position) {
        self.cursor.position = self.move_position(self.cursor.position, delta);
    }

    pub fn jump_cursor(&mut self, position: Position) {
        self.cursor.position = self.clamp_position(position);
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

        if cursor_effect_end.y <= self.cursor.position.y
            && cursor_effect_end.x <= self.cursor.position.x
        {
            self.cursor.position.x -= cursor_effect_end.x - start.x;
            self.cursor.position.y -= cursor_effect_end.y - start.y;
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
        if start.y <= self.cursor.position.y && start.x <= self.cursor.position.x {
            self.cursor.position.x += position.x - start.x;
            self.cursor.position.y += position.y - start.y;
        } else if start.y < self.cursor.position.y {
            self.cursor.position.y += position.y - start.y;
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

    pub fn position_to_visual(&self, position: Position, gfx: &Gfx) -> VisualPosition {
        let position = self.clamp_position(position);
        let leading_text = &self.lines[position.y as usize][..position.x as usize];

        let visual_x = Gfx::measure_text(leading_text.iter().copied());

        VisualPosition::new(
            visual_x as f32 * gfx.glyph_width(),
            position.y as f32 * gfx.line_height(),
        )
    }
}
