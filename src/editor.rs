use std::fs::File;

use crate::{
    action_history::ActionKind,
    cursor_index::CursorIndex,
    doc::Doc,
    gfx::{Color, Gfx},
    key::Key,
    keybind::{Keybind, MOD_ALT, MOD_CTRL, MOD_SHIFT},
    line_pool::LinePool,
    mouse_button::MouseButton,
    mousebind::Mousebind,
    position::Position,
    selection::Selection,
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
}

impl Editor {
    pub fn new(line_pool: &mut LinePool) -> Self {
        Self {
            doc: Doc::new(line_pool),
            camera_y: 0.0,
            camera_velocity_y: 0.0,
            camera_needs_recenter: false,
        }
    }

    pub fn is_animating(&self) -> bool {
        self.camera_velocity_y != 0.0
    }

    pub fn update(&mut self, window: &mut Window, line_pool: &mut LinePool, time: f32, dt: f32) {
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
                    } else {
                        self.doc.move_cursors(direction, mods & MOD_SHIFT != 0);
                    }
                }
                Keybind {
                    key: Key::Backspace,
                    mods: 0,
                } => {
                    for index in self.doc.cursor_indices() {
                        let cursor = self.doc.get_cursor(index);

                        let (start, end) = if let Some(selection) = cursor.get_selection() {
                            (selection.start, selection.end)
                        } else {
                            let end = cursor.position;
                            let start = self.doc.move_position(end, Position::new(-1, 0));

                            (start, end)
                        };

                        self.doc.delete(start, end, line_pool, time);
                        self.doc.end_cursor_selection(index);
                    }
                }
                Keybind {
                    key: Key::Delete,
                    mods: 0,
                } => {
                    for index in self.doc.cursor_indices() {
                        let cursor = self.doc.get_cursor(index);

                        let (start, end) = if let Some(selection) = cursor.get_selection() {
                            (selection.start, selection.end)
                        } else {
                            let start = cursor.position;
                            let end = self.doc.move_position(start, Position::new(1, 0));

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
                    self.doc
                        .load(&mut File::open("test.txt").unwrap(), line_pool)
                        .unwrap();
                }
                Keybind {
                    key: Key::S,
                    mods: MOD_CTRL,
                } => {
                    self.doc
                        .save(&mut File::create("test.txt").unwrap())
                        .unwrap();
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
                        let mut position = self.doc.get_cursor(index).position;
                        position.x = 0;

                        self.doc.jump_cursor(index, position, mods & MOD_SHIFT != 0);
                    }
                }
                Keybind {
                    key: Key::End,
                    mods,
                } => {
                    for index in self.doc.cursor_indices() {
                        let mut position = self.doc.get_cursor(index).position;
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

    pub fn draw(&mut self, gfx: &mut Gfx) {
        gfx.begin(None);

        let camera_y = self.camera_y.floor();
        let line_padding = (gfx.line_height() - gfx.glyph_height()) / 2.0;

        let min_y = (camera_y / gfx.line_height()) as usize;
        let sub_line_offset_y = camera_y - min_y as f32 * gfx.line_height();

        let max_y = ((camera_y + gfx.height()) / gfx.line_height()) as usize + 1;
        let max_y = max_y.min(self.doc.lines().len());

        for (i, line) in self.doc.lines()[min_y..max_y].iter().enumerate() {
            let y = i as f32 * gfx.line_height();

            gfx.add_text(
                line.iter().copied(),
                0.0,
                y + line_padding - sub_line_offset_y,
                &Color::new(0, 0, 0, 255),
            );
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
}
