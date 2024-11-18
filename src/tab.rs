use crate::{
    action_history::ActionKind,
    cursor_index::CursorIndex,
    doc::Doc,
    gfx::Gfx,
    key::Key,
    keybind::{Keybind, MOD_ALT, MOD_CTRL, MOD_SHIFT},
    line_pool::LinePool,
    mouse_button::MouseButton,
    mousebind::Mousebind,
    position::Position,
    rect::Rect,
    syntax_highlighter::Syntax,
    temp_buffer::TempBuffer,
    theme::Theme,
    visual_position::VisualPosition,
    window::Window,
};

const SCROLL_SPEED: f32 = 30.0;
const SCROLL_FRICTION: f32 = 0.0001;
const RECENTER_DISTANCE: usize = 3;

pub struct Tab {
    doc_index: usize,

    camera_y: f32,
    camera_velocity_y: f32,
    camera_needs_recenter: bool,

    tab_bounds: Rect,
    doc_bounds: Rect,
}

impl Tab {
    pub fn new(doc_index: usize) -> Self {
        Self {
            doc_index,

            camera_y: 0.0,
            camera_velocity_y: 0.0,
            camera_needs_recenter: false,

            tab_bounds: Rect::zero(),
            doc_bounds: Rect::zero(),
        }
    }

    pub fn doc_index(&self) -> usize {
        self.doc_index
    }

    pub fn is_animating(&self) -> bool {
        self.camera_velocity_y != 0.0
    }

    pub fn layout(&mut self, tab_bounds: Rect, doc_bounds: Rect) {
        self.tab_bounds = tab_bounds;
        self.doc_bounds = doc_bounds;
    }

    pub fn update(
        &mut self,
        doc: &mut Doc,
        window: &mut Window,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        syntax: Option<&Syntax>,
        time: f32,
        dt: f32,
    ) {
        let old_cursor_position = doc.get_cursor(CursorIndex::Main).position;

        let mut char_handler = window.get_char_handler();

        while let Some(char) = char_handler.next(window) {
            doc.insert_at_cursors(&[char], line_pool, time);
        }

        let mut mousebind_handler = window.get_mousebind_handler();

        while let Some(mousebind) = mousebind_handler.next(window) {
            let visual_position = VisualPosition::new(
                mousebind.x - self.doc_bounds.x,
                mousebind.y - self.doc_bounds.y,
            );

            let position = doc.visual_to_position(visual_position, self.camera_y, window.gfx());

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: 0 | MOD_SHIFT,
                    is_drag,
                    ..
                } => {
                    let mods = mousebind.mods;

                    doc.jump_cursors(position, is_drag || (mods & MOD_SHIFT != 0));
                }
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: MOD_CTRL,
                    is_drag: false,
                    ..
                } => {
                    doc.add_cursor(position);
                }
                _ => mousebind_handler.unprocessed(window, mousebind),
            }
        }

        let mut mouse_scroll_handler = window.get_mouse_scroll_handler();

        while let Some(mouse_scroll) = mouse_scroll_handler.next(window) {
            self.camera_needs_recenter = false;
            self.camera_velocity_y -=
                mouse_scroll.delta * window.gfx().line_height() * SCROLL_SPEED;
        }

        let mut keybind_handler = window.get_keybind_handler();

        while let Some(keybind) = keybind_handler.next(window) {
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
                        let cursor = doc.get_cursor(CursorIndex::Main);

                        let position = doc.move_position_with_desired_visual_x(
                            cursor.position,
                            direction,
                            Some(cursor.desired_visual_x),
                        );

                        doc.add_cursor(position);
                    } else if mods & MOD_CTRL != 0 {
                        match key {
                            Key::Up => doc.move_cursors_to_next_paragraph(-1, should_select),
                            Key::Down => doc.move_cursors_to_next_paragraph(1, should_select),
                            Key::Left => doc.move_cursors_to_next_word(-1, should_select),
                            Key::Right => doc.move_cursors_to_next_word(1, should_select),
                            _ => unreachable!(),
                        }
                    } else {
                        doc.move_cursors(direction, should_select);
                    }
                }
                Keybind {
                    key: Key::Backspace,
                    mods,
                } => {
                    for index in doc.cursor_indices() {
                        let cursor = doc.get_cursor(index);

                        let (start, end) = if let Some(selection) = cursor.get_selection() {
                            (selection.start, selection.end)
                        } else {
                            let end = cursor.position;

                            let start = if mods & MOD_CTRL != 0 {
                                doc.move_position_to_next_word(end, -1)
                            } else {
                                doc.move_position(end, Position::new(-1, 0))
                            };

                            (start, end)
                        };

                        doc.delete(start, end, line_pool, time);
                        doc.end_cursor_selection(index);
                    }
                }
                Keybind {
                    key: Key::Delete,
                    mods,
                } => {
                    for index in doc.cursor_indices() {
                        let cursor = doc.get_cursor(index);

                        let (start, end) = if let Some(selection) = cursor.get_selection() {
                            (selection.start, selection.end)
                        } else {
                            let start = cursor.position;

                            let end = if mods & MOD_CTRL != 0 {
                                doc.move_position_to_next_word(start, 1)
                            } else {
                                doc.move_position(start, Position::new(1, 0))
                            };

                            (start, end)
                        };

                        doc.delete(start, end, line_pool, time);
                        doc.end_cursor_selection(index);
                    }
                }
                Keybind {
                    key: Key::Enter,
                    mods: 0,
                } => {
                    let mut text_buffer = text_buffer.get_mut();

                    for index in doc.cursor_indices() {
                        let cursor = doc.get_cursor(index);

                        let mut indent_position = Position::new(0, cursor.position.y);

                        while indent_position < cursor.position {
                            let c = doc.get_char(indent_position);

                            if c.is_whitespace() {
                                text_buffer.push(c);
                                indent_position =
                                    doc.move_position(indent_position, Position::new(1, 0));
                            } else {
                                break;
                            }
                        }

                        doc.insert_at_cursor(index, &['\n'], line_pool, time);
                        doc.insert_at_cursor(index, &text_buffer, line_pool, time);
                    }
                }
                Keybind {
                    key: Key::Tab,
                    mods: 0,
                } => {
                    doc.insert_at_cursors(&['\t'], line_pool, time);
                }
                Keybind {
                    key: Key::PageUp,
                    mods: 0 | MOD_SHIFT,
                } => {
                    let mods = keybind.mods;
                    let height_lines = window.gfx().height_lines();

                    doc.move_cursors(Position::new(0, -height_lines), mods & MOD_SHIFT != 0);
                }
                Keybind {
                    key: Key::PageDown,
                    mods: 0 | MOD_SHIFT,
                } => {
                    let mods = keybind.mods;
                    let height_lines = window.gfx().height_lines();

                    doc.move_cursors(Position::new(0, height_lines), mods & MOD_SHIFT != 0);
                }
                Keybind {
                    key: Key::Home,
                    mods,
                } => {
                    for index in doc.cursor_indices() {
                        let mut position = if mods & MOD_CTRL != 0 {
                            Position::new(0, 0)
                        } else {
                            doc.get_cursor(index).position
                        };

                        position.x = 0;

                        doc.jump_cursor(index, position, mods & MOD_SHIFT != 0);
                    }
                }
                Keybind {
                    key: Key::End,
                    mods,
                } => {
                    for index in doc.cursor_indices() {
                        let mut position = if mods & MOD_CTRL != 0 {
                            Position::new(0, doc.lines().len() as isize - 1)
                        } else {
                            doc.get_cursor(index).position
                        };

                        position.x = doc.get_line_len(position.y);

                        doc.jump_cursor(index, position, mods & MOD_SHIFT != 0);
                    }
                }
                Keybind {
                    key: Key::A,
                    mods: MOD_CTRL,
                } => {
                    let y = doc.lines().len() as isize - 1;
                    let x = doc.get_line_len(y);

                    doc.jump_cursors(Position::zero(), false);
                    doc.jump_cursors(Position::new(x, y), true);
                }
                Keybind {
                    key: Key::Escape,
                    mods: 0,
                } => {
                    if doc.cursors_len() > 1 {
                        doc.clear_extra_cursors(CursorIndex::Some(0));
                    } else {
                        doc.end_cursor_selection(CursorIndex::Main);
                    }
                }
                Keybind {
                    key: Key::Z,
                    mods: MOD_CTRL,
                } => {
                    doc.undo(line_pool, ActionKind::Done);
                }
                Keybind {
                    key: Key::Y,
                    mods: MOD_CTRL,
                } => {
                    doc.undo(line_pool, ActionKind::Undone);
                }
                Keybind {
                    key: Key::C,
                    mods: MOD_CTRL,
                } => {
                    let mut text_buffer = text_buffer.get_mut();
                    let was_copy_implicit = doc.copy_at_cursors(&mut text_buffer);

                    let _ = window.set_clipboard(&text_buffer, was_copy_implicit);
                }
                Keybind {
                    key: Key::X,
                    mods: MOD_CTRL,
                } => {
                    let mut text_buffer = text_buffer.get_mut();
                    let was_copy_implicit = doc.copy_at_cursors(&mut text_buffer);

                    let _ = window.set_clipboard(&text_buffer, was_copy_implicit);

                    for index in doc.cursor_indices() {
                        let cursor = doc.get_cursor(index);

                        let (start, end) = if let Some(selection) = cursor.get_selection() {
                            (selection.start, selection.end)
                        } else {
                            let mut start = Position::new(0, cursor.position.y);
                            let mut end = Position::new(doc.get_line_len(start.y), start.y);

                            if start.y as usize == doc.lines().len() - 1 {
                                start = doc.move_position(start, Position::new(-1, 0));
                            } else {
                                end = doc.move_position(end, Position::new(1, 0));
                            }

                            (start, end)
                        };

                        doc.delete(start, end, line_pool, time);
                        doc.end_cursor_selection(index);
                    }
                }
                Keybind {
                    key: Key::V,
                    mods: MOD_CTRL,
                } => {
                    let was_copy_implicit = window.was_copy_implicit();
                    let text = window.get_clipboard().unwrap_or(&[]);

                    doc.paste_at_cursors(text, was_copy_implicit, line_pool, time);
                }
                _ => keybind_handler.unprocessed(window, keybind),
            }
        }

        doc.combine_overlapping_cursors();
        self.update_camera(doc, window, old_cursor_position, dt);

        if let Some(syntax) = syntax {
            doc.update_highlights(self.camera_y, window.gfx(), syntax);
        }
    }

    fn update_camera(
        &mut self,
        doc: &Doc,
        window: &mut Window,
        old_cursor_position: Position,
        dt: f32,
    ) {
        let gfx = window.gfx();

        let new_cursor_position = doc.get_cursor(CursorIndex::Main).position;
        let new_cursor_visual_position =
            doc.position_to_visual(new_cursor_position, self.camera_y, gfx);
        let cursor_scroll_border = gfx.line_height() * RECENTER_DISTANCE as f32;

        if old_cursor_position != new_cursor_position {
            self.camera_needs_recenter = new_cursor_visual_position.y < cursor_scroll_border
                || new_cursor_visual_position.y
                    > self.doc_bounds.height - gfx.line_height() - cursor_scroll_border;
        }

        if self.camera_needs_recenter {
            let visual_distance = if new_cursor_visual_position.y < self.doc_bounds.height / 2.0 {
                new_cursor_visual_position.y - cursor_scroll_border
            } else {
                new_cursor_visual_position.y - self.doc_bounds.height
                    + gfx.line_height()
                    + cursor_scroll_border
            };

            // We can't move the camera past the top of the document,
            // (eg. if the cursor is on the first line, it might be too close to the edge of the
            // screen according to RECENTER_DISTANCE, but there's nothing we can do about it, so stop animating).
            let visual_distance = (visual_distance + self.camera_y).max(0.0) - self.camera_y;

            self.scroll_visual_distance(visual_distance);
        }

        self.camera_velocity_y *= SCROLL_FRICTION.powf(dt);

        // We want the velocity to eventually be exactly zero so that we can stop animating.
        if self.camera_velocity_y.abs() < 0.5 {
            self.camera_velocity_y = 0.0;

            // If we're recentering the camera then we must be done at this point.
            self.camera_needs_recenter = false;
        }

        self.camera_y += self.camera_velocity_y * dt;
        self.camera_y = self.camera_y.clamp(
            0.0,
            (doc.lines().len() - 1) as f32 * window.gfx().line_height(),
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

    pub fn tab_bounds(&self) -> Rect {
        self.tab_bounds
    }

    pub fn doc_bounds(&self) -> Rect {
        self.doc_bounds
    }

    pub fn draw(&mut self, doc: &Doc, theme: &Theme, gfx: &mut Gfx, is_focused: bool) {
        gfx.begin(Some(self.doc_bounds));

        let camera_y = self.camera_y.floor();

        let min_y = (camera_y / gfx.line_height()) as usize;
        let sub_line_offset_y = camera_y - min_y as f32 * gfx.line_height();

        let max_y = ((camera_y + gfx.height()) / gfx.line_height()) as usize + 1;
        let max_y = max_y.min(doc.lines().len());

        let lines = doc.lines();
        let highlighted_lines = doc.highlighted_lines();

        for (i, y) in (min_y..max_y).enumerate() {
            let line = &lines[y];

            let visual_y = i as f32 * gfx.line_height() + gfx.line_padding() - sub_line_offset_y;

            if y >= highlighted_lines.len() {
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

        for index in doc.cursor_indices() {
            let Some(selection) = doc.get_cursor(index).get_selection() else {
                continue;
            };

            let start = selection.start.max(Position::new(0, min_y as isize));
            let end = selection.end.min(Position::new(0, max_y as isize));
            let mut position = start;

            while position < end {
                let highlight_position = doc.position_to_visual(position, camera_y, gfx);

                let char = doc.get_char(position);
                let char_width = Gfx::get_char_width(char);

                gfx.add_rect(
                    Rect::new(
                        highlight_position.x,
                        highlight_position.y,
                        char_width as f32 * gfx.glyph_width(),
                        gfx.line_height(),
                    ),
                    &theme.selection,
                );

                position = doc.move_position(position, Position::new(1, 0));
            }
        }

        if is_focused {
            for index in doc.cursor_indices() {
                let cursor_position =
                    doc.position_to_visual(doc.get_cursor(index).position, camera_y, gfx);

                gfx.add_rect(
                    Rect::new(
                        cursor_position.x,
                        cursor_position.y,
                        (gfx.glyph_width() * 0.25).ceil(),
                        gfx.line_height(),
                    ),
                    &theme.normal,
                );
            }
        }

        gfx.end();
    }
}
