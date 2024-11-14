use std::fs::File;

use crate::{
    doc::Doc,
    gfx::{Color, Gfx},
    key::Key,
    keybind::{Keybind, MOD_CTRL, MOD_SHIFT},
    line_pool::LinePool,
    mouse_button::MouseButton,
    mousebind::Mousebind,
    position::Position,
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

    pub fn update(&mut self, window: &mut Window, line_pool: &mut LinePool, dt: f32) {
        let old_cursor_position = self.doc.get_cursor().position;

        while let Some(char) = window.get_typed_char() {
            self.doc.insert_at_cursor(&[char], line_pool);
        }

        while let Some(keybind) = window.get_typed_keybind() {
            match keybind {
                Keybind {
                    key: Key::Left,
                    mods,
                } => self
                    .doc
                    .move_cursor(Position::new(-1, 0), mods & MOD_SHIFT != 0),
                Keybind {
                    key: Key::Right,
                    mods,
                } => {
                    self.doc
                        .move_cursor(Position::new(1, 0), mods & MOD_SHIFT != 0);
                }
                Keybind { key: Key::Up, mods } => {
                    self.doc
                        .move_cursor(Position::new(0, -1), mods & MOD_SHIFT != 0);
                }
                Keybind {
                    key: Key::Down,
                    mods,
                } => {
                    self.doc
                        .move_cursor(Position::new(0, 1), mods & MOD_SHIFT != 0);
                }
                Keybind {
                    key: Key::Backspace,
                    mods: 0,
                } => {
                    let (start, end) =
                        if let Some(selection) = self.doc.get_cursor().get_selection() {
                            self.doc.end_cursor_selection();

                            (selection.start, selection.end)
                        } else {
                            let end = self.doc.get_cursor().position;
                            let start = self.doc.move_position(end, Position::new(-1, 0));

                            (start, end)
                        };

                    self.doc.delete(start, end, line_pool);
                }
                Keybind {
                    key: Key::Delete,
                    mods: 0,
                } => {
                    let (start, end) =
                        if let Some(selection) = self.doc.get_cursor().get_selection() {
                            self.doc.end_cursor_selection();

                            (selection.start, selection.end)
                        } else {
                            let start = self.doc.get_cursor().position;
                            let end = self.doc.move_position(start, Position::new(1, 0));

                            (start, end)
                        };

                    self.doc.delete(start, end, line_pool);
                }
                Keybind {
                    key: Key::Enter,
                    mods: 0,
                } => {
                    self.doc.insert_at_cursor(&['\n'], line_pool);
                }
                Keybind {
                    key: Key::Tab,
                    mods: 0,
                } => {
                    self.doc.insert_at_cursor(&['\t'], line_pool);
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
                        .move_cursor(Position::new(0, -height_lines), mods & MOD_SHIFT != 0);
                }
                Keybind {
                    key: Key::PageDown,
                    mods,
                } => {
                    let height_lines = window.gfx().height_lines();

                    self.doc
                        .move_cursor(Position::new(0, height_lines), mods & MOD_SHIFT != 0);
                }
                Keybind {
                    key: Key::Home,
                    mods,
                } => {
                    let mut position = self.doc.get_cursor().position;
                    position.x = 0;

                    self.doc.jump_cursor(position, mods & MOD_SHIFT != 0);
                }
                Keybind {
                    key: Key::End,
                    mods,
                } => {
                    let mut position = self.doc.get_cursor().position;
                    position.x = self.doc.get_line_len(position.y);

                    self.doc.jump_cursor(position, mods & MOD_SHIFT != 0);
                }
                Keybind {
                    key: Key::A,
                    mods: MOD_CTRL,
                } => {
                    let y = self.doc.lines().len() as isize - 1;
                    let x = self.doc.get_line_len(y);

                    self.doc.jump_cursor(Position::zero(), false);
                    self.doc.jump_cursor(Position::new(x, y), true);
                }
                _ => {}
            }
        }

        while let Some(mousebind) = window.get_pressed_mousebind() {
            match mousebind {
                Mousebind {
                    button: MouseButton::Left,
                    x,
                    y,
                    mods: 0,
                    is_drag,
                } => {
                    let position = self.doc.visual_to_position(
                        VisualPosition::new(x, y),
                        self.camera_y,
                        &window.gfx(),
                    );

                    self.doc.jump_cursor(position, is_drag);
                }
                _ => {}
            }
        }

        while let Some(mouse_scroll) = window.get_mouse_scroll() {
            self.camera_needs_recenter = false;
            self.camera_velocity_y -=
                mouse_scroll.delta * window.gfx().line_height() * SCROLL_SPEED;
        }

        let gfx = window.gfx();

        let new_cursor_position = self.doc.get_cursor().position;
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

        if let Some(selection) = self.doc.get_cursor().get_selection() {
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

        let cursor_position =
            self.doc
                .position_to_visual(self.doc.get_cursor().position, camera_y, gfx);

        gfx.add_rect(
            cursor_position.x,
            cursor_position.y,
            (gfx.glyph_width() * 0.25).ceil(),
            gfx.line_height(),
            &Color::new(0, 0, 0, 255),
        );

        gfx.end();
    }
}
