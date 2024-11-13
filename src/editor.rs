use crate::{
    doc::Doc,
    gfx::{Color, Gfx},
    key::Key,
    keybind::Keybind,
    line_pool::LinePool,
    mouse_button::MouseButton,
    mousebind::Mousebind,
    position::Position,
    visual_position::VisualPosition,
    window::Window,
};

pub struct Editor {
    doc: Doc,
}

impl Editor {
    pub fn new(line_pool: &mut LinePool) -> Self {
        Self {
            doc: Doc::new(line_pool),
        }
    }

    pub fn update(&mut self, window: &mut Window, line_pool: &mut LinePool) {
        while let Some(char) = window.get_typed_char() {
            let start = self.doc.get_cursor().position;
            self.doc.insert(start, &[char], line_pool);
        }

        while let Some(keybind) = window.get_typed_keybind() {
            match keybind {
                Keybind {
                    key: Key::Left,
                    mods: 0,
                } => {
                    self.doc.move_cursor(Position::new(-1, 0));
                }
                Keybind {
                    key: Key::Right,
                    mods: 0,
                } => {
                    self.doc.move_cursor(Position::new(1, 0));
                }
                Keybind {
                    key: Key::Up,
                    mods: 0,
                } => {
                    self.doc.move_cursor(Position::new(0, -1));
                }
                Keybind {
                    key: Key::Down,
                    mods: 0,
                } => {
                    self.doc.move_cursor(Position::new(0, 1));
                }
                Keybind {
                    key: Key::Backspace,
                    mods: 0,
                } => {
                    let end = self.doc.get_cursor().position;
                    let start = self.doc.move_position(end, Position::new(-1, 0));

                    self.doc.delete(start, end, line_pool);
                }
                Keybind {
                    key: Key::Delete,
                    mods: 0,
                } => {
                    let start = self.doc.get_cursor().position;
                    let end = self.doc.move_position(start, Position::new(1, 0));

                    self.doc.delete(start, end, line_pool);
                }
                Keybind {
                    key: Key::Enter,
                    mods: 0,
                } => {
                    let start = self.doc.get_cursor().position;
                    self.doc.insert(start, &['\n'], line_pool);
                }
                Keybind {
                    key: Key::Tab,
                    mods: 0,
                } => {
                    let start = self.doc.get_cursor().position;
                    // self.doc.insert(start, &[' ', ' ', ' ', ' '], line_pool);
                    self.doc.insert(start, &['\t'], line_pool);
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
                    ..
                } => {
                    let position = self
                        .doc
                        .visual_to_position(VisualPosition::new(x, y), &window.gfx());

                    self.doc.jump_cursor(position);
                }
                _ => {}
            }
        }
    }

    pub fn draw(&mut self, gfx: &mut Gfx) {
        gfx.begin(None);

        let cursor_position = self
            .doc
            .position_to_visual(self.doc.get_cursor().position, gfx);

        gfx.add_rect(
            cursor_position.x,
            cursor_position.y,
            gfx.glyph_width(),
            gfx.glyph_height(),
            &Color::new(125, 125, 200, 255),
        );

        for (i, line) in self.doc.lines().iter().enumerate() {
            let y = i as f32 * gfx.line_height();

            gfx.add_text(line.iter().copied(), 0.0, y, &Color::new(0, 0, 0, 255));
        }

        gfx.end();
    }
}
