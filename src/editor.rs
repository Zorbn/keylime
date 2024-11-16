use crate::{
    action_history::ActionKind,
    cursor_index::CursorIndex,
    dialog::{find_file, message, FindFileKind, MessageKind, MessageResponse},
    doc::Doc,
    gfx::{Color, Gfx},
    key::Key,
    keybind::{Keybind, MOD_ALT, MOD_CTRL, MOD_SHIFT},
    line_pool::LinePool,
    mouse_button::MouseButton,
    mousebind::Mousebind,
    position::Position,
    selection::Selection,
    syntax_highlighter::Syntax,
    theme::Theme,
    visual_position::VisualPosition,
    window::Window,
};

const SCROLL_SPEED: f32 = 30.0;
const SCROLL_FRICTION: f32 = 0.0001;
const RECENTER_DISTANCE: usize = 3;

pub struct Editor {
    doc: Doc,
    camera_y: f32,
    camera_velocity_y: f32,
    camera_needs_recenter: bool,
    copied_text: Vec<char>,
}

impl Editor {
    pub fn new(line_pool: &mut LinePool) -> Self {
        Self {
            doc: Doc::new(line_pool),
            camera_y: 0.0,
            camera_velocity_y: 0.0,
            camera_needs_recenter: false,
            copied_text: Vec::new(),
        }
    }

    pub fn is_animating(&self) -> bool {
        self.camera_velocity_y != 0.0
    }

    pub fn update(
        &mut self,
        window: &mut Window,
        line_pool: &mut LinePool,
        syntax: &Syntax,
        time: f32,
        dt: f32,
    ) {
        let old_cursor_position = self.doc.get_cursor(CursorIndex::Main).position;

        while let Some(char) = window.get_typed_char() {
            self.doc.insert_at_cursors(&[char], line_pool, time);
        }

        while let Some(keybind) = window.get_typed_keybind() {
            match keybind {
                Keybind {
                    key: Key::Up | Key::Down | Key::Left | Key::Right,
                    mods,
                } => {
                    let key = keybind.key;

                    let direction = match key {
                        Key::Up => Position::new(0, -1),
                        Key::Down => Position::new(0, 1),
                        Key::Left => Position::new(-1, 0),
                        Key::Right => Position::new(1, 0),
                        _ => unreachable!(),
                    };

                    let should_select = mods & MOD_SHIFT != 0;

                    if (mods & MOD_CTRL != 0)
                        && (mods & MOD_ALT != 0)
                        && (key == Key::Up || key == Key::Down)
                    {
                        let cursor = self.doc.get_cursor(CursorIndex::Main);

                        let position = self.doc.move_position_with_desired_visual_x(
                            cursor.position,
                            direction,
                            Some(cursor.desired_visual_x),
                        );

                        self.doc.add_cursor(position);
                    } else if mods & MOD_CTRL != 0 {
                        match key {
                            Key::Up => self.doc.move_cursors_to_next_paragraph(-1, should_select),
                            Key::Down => self.doc.move_cursors_to_next_paragraph(1, should_select),
                            Key::Left => self.doc.move_cursors_to_next_word(-1, should_select),
                            Key::Right => self.doc.move_cursors_to_next_word(1, should_select),
                            _ => unreachable!(),
                        }
                    } else {
                        self.doc.move_cursors(direction, should_select);
                    }
                }
                Keybind {
                    key: Key::Backspace,
                    mods,
                } => {
                    for index in self.doc.cursor_indices() {
                        let cursor = self.doc.get_cursor(index);

                        let (start, end) = if let Some(selection) = cursor.get_selection() {
                            (selection.start, selection.end)
                        } else {
                            let end = cursor.position;

                            let start = if mods & MOD_CTRL != 0 {
                                self.doc.move_position_to_next_word(end, -1, false)
                            } else {
                                self.doc.move_position(end, Position::new(-1, 0))
                            };

                            (start, end)
                        };

                        self.doc.delete(start, end, line_pool, time);
                        self.doc.end_cursor_selection(index);
                    }
                }
                Keybind {
                    key: Key::Delete,
                    mods,
                } => {
                    for index in self.doc.cursor_indices() {
                        let cursor = self.doc.get_cursor(index);

                        let (start, end) = if let Some(selection) = cursor.get_selection() {
                            (selection.start, selection.end)
                        } else {
                            let start = cursor.position;

                            let end = if mods & MOD_CTRL != 0 {
                                self.doc.move_position_to_next_word(start, 1, false)
                            } else {
                                self.doc.move_position(start, Position::new(1, 0))
                            };

                            (start, end)
                        };

                        self.doc.delete(start, end, line_pool, time);
                        self.doc.end_cursor_selection(index);
                    }
                }
                Keybind {
                    key: Key::Enter,
                    mods: 0,
                } => {
                    self.doc.insert_at_cursors(&['\n'], line_pool, time);
                }
                Keybind {
                    key: Key::Tab,
                    mods: 0,
                } => {
                    self.doc.insert_at_cursors(&['\t'], line_pool, time);
                }
                Keybind {
                    key: Key::O,
                    mods: MOD_CTRL,
                } => {
                    if let Ok(path) = find_file(FindFileKind::OpenFile) {
                        if let Err(err) = self.doc.load(&path, line_pool) {
                            message("Failed to Open File", &err.to_string(), MessageKind::Ok);
                        }
                    }
                }
                Keybind {
                    key: Key::S,
                    mods: MOD_CTRL,
                } => {
                    self.try_save();
                }
                Keybind {
                    key: Key::W,
                    mods: MOD_CTRL,
                } => {
                    if self.confirm_close("closing") {
                        self.doc = Doc::new(line_pool);
                    }
                }
                Keybind {
                    key: Key::R,
                    mods: MOD_CTRL,
                } => {
                    if self.confirm_close("reloading") {
                        if let Some(path) = self.doc.path().cloned() {
                            if let Err(err) = self.doc.load(&path, line_pool) {
                                message("Failed to Reload File", &err.to_string(), MessageKind::Ok);
                            }
                        }
                    }
                }
                Keybind {
                    key: Key::PageUp,
                    mods,
                } => {
                    let height_lines = window.gfx().height_lines();

                    self.doc
                        .move_cursors(Position::new(0, -height_lines), mods & MOD_SHIFT != 0);
                }
                Keybind {
                    key: Key::PageDown,
                    mods,
                } => {
                    let height_lines = window.gfx().height_lines();

                    self.doc
                        .move_cursors(Position::new(0, height_lines), mods & MOD_SHIFT != 0);
                }
                Keybind {
                    key: Key::Home,
                    mods,
                } => {
                    for index in self.doc.cursor_indices() {
                        let mut position = if mods & MOD_CTRL != 0 {
                            Position::new(0, 0)
                        } else {
                            self.doc.get_cursor(index).position
                        };

                        position.x = 0;

                        self.doc.jump_cursor(index, position, mods & MOD_SHIFT != 0);
                    }
                }
                Keybind {
                    key: Key::End,
                    mods,
                } => {
                    for index in self.doc.cursor_indices() {
                        let mut position = if mods & MOD_CTRL != 0 {
                            Position::new(0, self.doc.lines().len() as isize - 1)
                        } else {
                            self.doc.get_cursor(index).position
                        };

                        position.x = self.doc.get_line_len(position.y);

                        self.doc.jump_cursor(index, position, mods & MOD_SHIFT != 0);
                    }
                }
                Keybind {
                    key: Key::A,
                    mods: MOD_CTRL,
                } => {
                    let y = self.doc.lines().len() as isize - 1;
                    let x = self.doc.get_line_len(y);

                    self.doc.jump_cursors(Position::zero(), false);
                    self.doc.jump_cursors(Position::new(x, y), true);
                }
                Keybind {
                    key: Key::Escape,
                    mods: 0,
                } => {
                    self.doc.clear_extra_cursors(CursorIndex::Some(0));
                }
                Keybind {
                    key: Key::Z,
                    mods: MOD_CTRL,
                } => {
                    self.doc.undo(line_pool, ActionKind::Done);
                }
                Keybind {
                    key: Key::Y,
                    mods: MOD_CTRL,
                } => {
                    self.doc.undo(line_pool, ActionKind::Undone);
                }
                Keybind {
                    key: Key::C,
                    mods: MOD_CTRL,
                } => {
                    self.copied_text.clear();
                    let was_copy_implicit = self.doc.copy_at_cursors(&mut self.copied_text);

                    let _ = window.set_clipboard(&self.copied_text, was_copy_implicit);
                }
                Keybind {
                    key: Key::X,
                    mods: MOD_CTRL,
                } => {
                    self.copied_text.clear();
                    let was_copy_implicit = self.doc.copy_at_cursors(&mut self.copied_text);

                    let _ = window.set_clipboard(&self.copied_text, was_copy_implicit);

                    for index in self.doc.cursor_indices() {
                        let cursor = self.doc.get_cursor(index);

                        let (start, end) = if let Some(selection) = cursor.get_selection() {
                            (selection.start, selection.end)
                        } else {
                            let mut start = Position::new(0, cursor.position.y);
                            let mut end = Position::new(self.doc.get_line_len(start.y), start.y);

                            if start.y as usize == self.doc.lines().len() - 1 {
                                start = self.doc.move_position(start, Position::new(-1, 0));
                            } else {
                                end = self.doc.move_position(end, Position::new(1, 0));
                            }

                            (start, end)
                        };

                        self.doc.delete(start, end, line_pool, time);
                        self.doc.end_cursor_selection(index);
                    }
                }
                Keybind {
                    key: Key::V,
                    mods: MOD_CTRL,
                } => {
                    let was_copy_implicit = window.was_copy_implicit();
                    let text = window.get_clipboard().unwrap_or(&[]);

                    self.doc
                        .paste_at_cursors(text, was_copy_implicit, line_pool, time);
                }
                _ => {}
            }
        }

        while let Some(mousebind) = window.get_pressed_mousebind() {
            let visual_position = VisualPosition::new(mousebind.x, mousebind.y);
            let position =
                self.doc
                    .visual_to_position(visual_position, self.camera_y, window.gfx());

            match mousebind {
                Mousebind {
                    button: MouseButton::Left,
                    mods: 0,
                    is_drag,
                    ..
                } => {
                    self.doc.jump_cursors(position, is_drag);
                }
                Mousebind {
                    button: MouseButton::Left,
                    mods: MOD_CTRL,
                    is_drag: false,
                    ..
                } => {
                    self.doc.add_cursor(position);
                }
                _ => {}
            }
        }

        while let Some(mouse_scroll) = window.get_mouse_scroll() {
            self.camera_needs_recenter = false;
            self.camera_velocity_y -=
                mouse_scroll.delta * window.gfx().line_height() * SCROLL_SPEED;
        }

        self.combine_overlapping_cursors();
        self.update_camera(window, old_cursor_position, dt);
        self.doc
            .update_highlights(self.camera_y, window.gfx(), syntax);
    }

    fn update_camera(&mut self, window: &mut Window, old_cursor_position: Position, dt: f32) {
        let gfx = window.gfx();

        let new_cursor_position = self.doc.get_cursor(CursorIndex::Main).position;
        let new_cursor_visual_position =
            self.doc
                .position_to_visual(new_cursor_position, self.camera_y, gfx);
        let cursor_scroll_border = gfx.line_height() * RECENTER_DISTANCE as f32;

        if old_cursor_position != new_cursor_position {
            self.camera_needs_recenter = new_cursor_visual_position.y < cursor_scroll_border
                || new_cursor_visual_position.y
                    > gfx.height() - gfx.line_height() - cursor_scroll_border;
        }

        if self.camera_needs_recenter {
            let visual_distance = if new_cursor_visual_position.y < gfx.height() / 2.0 {
                new_cursor_visual_position.y - cursor_scroll_border
            } else {
                new_cursor_visual_position.y - gfx.height()
                    + gfx.line_height()
                    + cursor_scroll_border
            };

            // We can't move the camera past the top of the document,
            // (eg. if the cursor is on the first line, it might be too close to the edge of the
            // screen according to RECENTER_DISTANCE, but there's nothing we can do about it, so stop animating).
            let visual_distance = (visual_distance + self.camera_y).max(0.0) - self.camera_y;

            self.scroll_visual_distance(visual_distance);
        }

        self.camera_velocity_y = self.camera_velocity_y * SCROLL_FRICTION.powf(dt);

        // We want the velocity to eventually be exactly zero so that we can stop animating.
        if self.camera_velocity_y.abs() < 0.5 {
            self.camera_velocity_y = 0.0;

            // If we're recentering the camera then we must be done at this point.
            self.camera_needs_recenter = false;
        }

        self.camera_y += self.camera_velocity_y * dt;
        self.camera_y = self.camera_y.clamp(
            0.0,
            (self.doc.lines().len() - 1) as f32 * window.gfx().line_height(),
        );
    }

    fn combine_overlapping_cursors(&mut self) {
        for index in self.doc.cursor_indices().rev() {
            let cursor = self.doc.get_cursor(index);
            let position = cursor.position;
            let selection = cursor.get_selection();

            for other_index in self.doc.cursor_indices() {
                if index == other_index {
                    continue;
                }

                let other_cursor = self.doc.get_cursor(other_index);

                let do_remove = if let Some(selection) = other_cursor.get_selection() {
                    position >= selection.start && position <= selection.end
                } else {
                    position == other_cursor.position
                };

                if !do_remove {
                    continue;
                }

                self.doc.set_cursor_selection(
                    other_index,
                    Selection::union(other_cursor.get_selection(), selection),
                );
                self.doc.remove_cursor(index);

                break;
            }
        }
    }

    fn scroll_visual_distance(&mut self, visual_distance: f32) {
        let f = SCROLL_FRICTION;
        let t = 1.0; // Time to scroll to destination.

        // Velocity of the camera is (v = starting velocity, f = friction factor): v * f^t
        // Integrate to get position: y = (v * f^t) / ln(f)
        // Add term so we start at zero: y = (v * f^t) / ln(f) - v / ln(f)
        // Solve for v:
        let v = (visual_distance * f.ln()) / (f.powf(t) - 1.0);

        self.camera_velocity_y = v;
    }

    pub fn draw(&mut self, theme: &Theme, gfx: &mut Gfx) {
        gfx.begin(None);

        let camera_y = self.camera_y.floor();
        let line_padding = (gfx.line_height() - gfx.glyph_height()) / 2.0;

        let min_y = (camera_y / gfx.line_height()) as usize;
        let sub_line_offset_y = camera_y - min_y as f32 * gfx.line_height();

        let max_y = ((camera_y + gfx.height()) / gfx.line_height()) as usize + 1;
        let max_y = max_y.min(self.doc.lines().len());

        let lines = self.doc.lines();
        let highlighted_lines = self.doc.highlighted_lines();

        for (i, y) in (min_y..max_y).enumerate() {
            let line = &lines[y];

            let visual_y = i as f32 * gfx.line_height() + line_padding - sub_line_offset_y;

            if y > highlighted_lines.len() {
                gfx.add_text(line.iter().copied(), 0.0, visual_y, &theme.normal);
            } else {
                let mut x = 0;
                let highlighted_line = &highlighted_lines[y];

                for highlight in highlighted_line.highlights() {
                    let color = &theme.highlight_kind_to_color(highlight.kind);

                    x += gfx.add_text(
                        line[highlight.start..highlight.end].iter().copied(),
                        x as f32 * gfx.glyph_width(),
                        visual_y,
                        color,
                    );
                }
            }
        }

        for index in self.doc.cursor_indices() {
            let Some(selection) = self.doc.get_cursor(index).get_selection() else {
                continue;
            };

            let start = selection.start.max(Position::new(0, min_y as isize));
            let end = selection.end.min(Position::new(0, max_y as isize));
            let mut position = start;

            while position < end {
                let highlight_position = self.doc.position_to_visual(position, camera_y, gfx);

                let char = self.doc.get_char(position);
                let char_width = Gfx::get_char_width(char);

                gfx.add_rect(
                    highlight_position.x,
                    highlight_position.y,
                    char_width as f32 * gfx.glyph_width(),
                    gfx.line_height(),
                    &Color::new(76, 173, 228, 125),
                );

                position = self.doc.move_position(position, Position::new(1, 0));
            }
        }

        for index in self.doc.cursor_indices() {
            let cursor_position =
                self.doc
                    .position_to_visual(self.doc.get_cursor(index).position, camera_y, gfx);

            gfx.add_rect(
                cursor_position.x,
                cursor_position.y,
                (gfx.glyph_width() * 0.25).ceil(),
                gfx.line_height(),
                &Color::new(0, 0, 0, 255),
            );
        }

        gfx.end();
    }

    fn confirm_close(&mut self, reason: &str) -> bool {
        if self.doc.is_saved() {
            true
        } else {
            let text = format!(
                "{} has unsaved changes. Do you want to save it before {}?",
                self.doc.file_name(),
                reason
            );

            match message("Unsaved Changes", &text, MessageKind::YesNoCancel) {
                MessageResponse::Yes => self.try_save(),
                MessageResponse::No => true,
                MessageResponse::Cancel => false,
            }
        }
    }

    fn try_save(&mut self) -> bool {
        let path = if let Some(path) = self.doc.path() {
            Ok(path.clone())
        } else {
            find_file(FindFileKind::Save)
        };

        if let Err(err) = path.map(|path| self.doc.save(path)) {
            message("Failed to Save File", &err.to_string(), MessageKind::Ok);
            false
        } else {
            true
        }
    }
}
