use core::f32;

use crate::{
    config::{theme::Theme, Config},
    digits::get_digits,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    input::{
        key::Key,
        keybind::{Keybind, MOD_ALT, MOD_CTRL, MOD_SHIFT},
        mouse_button::MouseButton,
        mousebind::Mousebind,
    },
    platform::{gfx::Gfx, window::Window},
    temp_buffer::TempBuffer,
    text::{
        action_history::ActionKind,
        cursor_index::CursorIndex,
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
    ui::camera::{Camera, RECENTER_DISTANCE},
};

const GUTTER_PADDING_WIDTH: f32 = 1.0;
const GUTTER_BORDER_WIDTH: f32 = 0.5;

pub struct Tab {
    doc_index: usize,

    pub camera: Camera,

    tab_bounds: Rect,
    gutter_bounds: Rect,
    doc_bounds: Rect,
}

impl Tab {
    pub fn new(doc_index: usize) -> Self {
        Self {
            doc_index,

            camera: Camera::new(),

            tab_bounds: Rect::zero(),
            gutter_bounds: Rect::zero(),
            doc_bounds: Rect::zero(),
        }
    }

    pub fn doc_index(&self) -> usize {
        self.doc_index
    }

    pub fn is_animating(&self) -> bool {
        self.camera.is_moving()
    }

    pub fn layout(&mut self, tab_bounds: Rect, doc_bounds: Rect, doc: &Doc, gfx: &Gfx) {
        self.tab_bounds = tab_bounds;

        let gutter_width = if doc.kind() == DocKind::MultiLine {
            let max_gutter_digits = (doc.lines().len() as f32).log10().floor() + 1.0;

            (max_gutter_digits + GUTTER_PADDING_WIDTH * 2.0 + GUTTER_BORDER_WIDTH)
                * gfx.glyph_width()
        } else {
            0.0
        };

        self.gutter_bounds = Rect::new(doc_bounds.x, doc_bounds.y, gutter_width, doc_bounds.height);
        self.doc_bounds = doc_bounds.shrink_left_by(self.gutter_bounds);
    }

    pub fn update(
        &mut self,
        doc: &mut Doc,
        window: &mut Window,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
        dt: f32,
    ) {
        let language = config.get_language_for_doc(doc);

        let mut handled_cursor_position = doc.get_cursor(CursorIndex::Main).position;

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

            let position = doc.visual_to_position(
                visual_position,
                self.camera.x(),
                self.camera.y(),
                window.gfx(),
            );

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: 0 | MOD_SHIFT,
                    is_drag,
                    ..
                } => {
                    let mods = mousebind.mods;

                    doc.jump_cursors(position, is_drag || (mods & MOD_SHIFT != 0));
                    handled_cursor_position = doc.get_cursor(CursorIndex::Main).position;
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
            let position = VisualPosition::new(mouse_scroll.x, mouse_scroll.y);

            if self.doc_bounds.contains_position(position) {
                let delta = mouse_scroll.delta * window.gfx().line_height();

                if mouse_scroll.is_horizontal {
                    self.camera.vertical.reset_velocity();
                    self.camera.horizontal.scroll(-delta);
                } else {
                    self.camera.horizontal.reset_velocity();
                    self.camera.vertical.scroll(delta);
                }
            }
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
                    let indent_width = language
                        .and_then(|language| language.indent_width)
                        .unwrap_or(1);

                    for index in doc.cursor_indices() {
                        let cursor = doc.get_cursor(index);

                        let (start, end) = if let Some(selection) = cursor.get_selection() {
                            (selection.start, selection.end)
                        } else {
                            let end = cursor.position;

                            let start = if mods & MOD_CTRL != 0 {
                                doc.move_position_to_next_word(end, -1)
                            } else {
                                let indent_width = (end.x - 1) % indent_width as isize + 1;
                                let mut start = doc.move_position(end, Position::new(-1, 0));

                                for _ in 1..indent_width {
                                    let next_start = doc.move_position(start, Position::new(-1, 0));

                                    if doc.get_char(next_start) != ' ' {
                                        break;
                                    }

                                    start = next_start;
                                }

                                start
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
                    mods,
                } => {
                    let indent_width = language.and_then(|language| language.indent_width);
                    let do_unindent = mods & MOD_SHIFT != 0;

                    for index in doc.cursor_indices() {
                        let cursor = doc.get_cursor(index);

                        if cursor.get_selection().is_some() || do_unindent {
                            doc.indent_lines_at_cursor(
                                index,
                                indent_width,
                                do_unindent,
                                line_pool,
                                time,
                            );
                        } else {
                            doc.indent_at_cursors(indent_width, line_pool, time);
                        }
                    }
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
                        let position = if mods & MOD_CTRL != 0 {
                            Position::new(0, 0)
                        } else {
                            let cursor = doc.get_cursor(index);
                            let line_start_x = doc.get_line_start(cursor.position.y);

                            let x = if line_start_x == cursor.position.x {
                                0
                            } else {
                                line_start_x
                            };

                            Position::new(x, cursor.position.y)
                        };

                        doc.jump_cursor(index, position, mods & MOD_SHIFT != 0);
                    }
                }
                Keybind {
                    key: Key::End,
                    mods,
                } => {
                    for index in doc.cursor_indices() {
                        let position = if mods & MOD_CTRL != 0 {
                            doc.end()
                        } else {
                            let mut position = doc.get_cursor(index).position;
                            position.x = doc.get_line_len(position.y);

                            position
                        };

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
                Keybind {
                    key: Key::D,
                    mods: MOD_CTRL,
                } => {
                    doc.add_cursor_at_next_occurance();
                }
                Keybind {
                    key: Key::ForwardSlash,
                    mods: MOD_CTRL,
                } => {
                    if let Some(language) = language {
                        doc.toggle_comments_at_cursors(&language.comment, line_pool, time);
                    }
                }
                Keybind {
                    key: Key::LBracket | Key::RBracket,
                    mods: MOD_CTRL,
                } => {
                    let indent_width = language.and_then(|language| language.indent_width);
                    let do_unindent = keybind.key == Key::LBracket;

                    doc.indent_lines_at_cursors(indent_width, do_unindent, line_pool, time);
                }
                _ => keybind_handler.unprocessed(window, keybind),
            }
        }

        doc.combine_overlapping_cursors();
        self.update_camera(doc, window, handled_cursor_position, dt);
        self.update_horizontal_camera(doc, window, handled_cursor_position, dt);
    }

    fn update_camera(
        &mut self,
        doc: &Doc,
        window: &mut Window,
        handled_cursor_position: Position,
        dt: f32,
    ) {
        let gfx = window.gfx();

        let new_cursor_position = doc.get_cursor(CursorIndex::Main).position;
        let new_cursor_visual_position =
            doc.position_to_visual(new_cursor_position, self.camera.x(), self.camera.y(), gfx);

        let can_recenter = handled_cursor_position != new_cursor_position;

        let target_y = new_cursor_visual_position.y + gfx.line_height() / 2.0;
        let max_y = (doc.lines().len() - 1) as f32 * gfx.line_height();

        let scroll_border_top = gfx.line_height() * RECENTER_DISTANCE as f32;
        let scroll_border_bottom = self.doc_bounds.height - scroll_border_top - gfx.line_height();

        self.camera.vertical.update(
            target_y,
            max_y,
            self.doc_bounds.height,
            scroll_border_top,
            scroll_border_bottom,
            can_recenter,
            dt,
        );
    }

    fn update_horizontal_camera(
        &mut self,
        doc: &Doc,
        window: &mut Window,
        handled_cursor_position: Position,
        dt: f32,
    ) {
        let gfx = window.gfx();

        let new_cursor_position = doc.get_cursor(CursorIndex::Main).position;
        let new_cursor_visual_position =
            doc.position_to_visual(new_cursor_position, self.camera.x(), self.camera.y(), gfx);

        let can_recenter = handled_cursor_position != new_cursor_position;

        let target_x = new_cursor_visual_position.x + gfx.glyph_width() / 2.0;
        let max_x = f32::MAX;

        let scroll_border_left = gfx.glyph_width() * RECENTER_DISTANCE as f32;
        let scroll_border_right = self.doc_bounds.width - scroll_border_left - gfx.glyph_width();

        self.camera.horizontal.update(
            target_x,
            max_x,
            self.doc_bounds.height,
            scroll_border_left,
            scroll_border_right,
            can_recenter,
            dt,
        );
    }

    pub fn tab_bounds(&self) -> Rect {
        self.tab_bounds
    }

    pub fn doc_bounds(&self) -> Rect {
        self.doc_bounds
    }

    fn get_line_visual_y(index: usize, sub_line_offset_y: f32, gfx: &Gfx) -> f32 {
        index as f32 * gfx.line_height() + gfx.line_padding() - sub_line_offset_y
    }

    pub fn draw(&mut self, doc: &mut Doc, config: &Config, gfx: &mut Gfx, is_focused: bool) {
        if let Some(syntax) = config
            .get_language_for_doc(doc)
            .and_then(|language| language.syntax.as_ref())
        {
            doc.update_highlights(
                self.camera.x(),
                self.camera.y(),
                self.doc_bounds,
                syntax,
                gfx,
            );
        }

        let camera_x = self.camera.x().floor();
        let camera_y = self.camera.y().floor();

        let min_y = (camera_y / gfx.line_height()) as usize;
        let sub_line_offset_y = camera_y - min_y as f32 * gfx.line_height();

        let max_y = ((camera_y + self.doc_bounds.height) / gfx.line_height()) as usize + 1;
        let max_y = max_y.min(doc.lines().len());

        if doc.kind() == DocKind::MultiLine {
            gfx.begin(Some(self.gutter_bounds));

            self.draw_gutter(doc, &config.theme, gfx, sub_line_offset_y, min_y, max_y);

            gfx.end();
        }

        gfx.begin(Some(self.doc_bounds));

        self.draw_lines(
            doc,
            &config.theme,
            gfx,
            camera_x,
            sub_line_offset_y,
            min_y,
            max_y,
        );
        self.draw_cursors(
            doc,
            &config.theme,
            gfx,
            is_focused,
            camera_x,
            camera_y,
            min_y,
            max_y,
        );

        gfx.end();
    }

    fn draw_gutter(
        &mut self,
        doc: &Doc,
        theme: &Theme,
        gfx: &mut Gfx,
        sub_line_offset_y: f32,
        min_y: usize,
        max_y: usize,
    ) {
        let cursor_y = doc.get_cursor(CursorIndex::Main).position.y;

        let mut digits = [' '; 20];

        for (i, y) in (min_y..max_y).enumerate() {
            let digits = get_digits(y + 1, &mut digits);
            let visual_y = Self::get_line_visual_y(i, sub_line_offset_y, gfx);

            let width = digits.len() as f32 * gfx.glyph_width();
            let visual_x = self.gutter_bounds.width
                - width
                - (GUTTER_PADDING_WIDTH + GUTTER_BORDER_WIDTH) * gfx.glyph_width();

            let color = if y as isize == cursor_y {
                &theme.normal
            } else {
                &theme.line_number
            };

            gfx.add_text(digits.iter().copied(), visual_x, visual_y, color);
        }

        gfx.add_rect(
            self.gutter_bounds
                .unoffset_by(self.gutter_bounds)
                .right_border(gfx.border_width()),
            &theme.border,
        );
    }

    fn draw_lines(
        &mut self,
        doc: &Doc,
        theme: &Theme,
        gfx: &mut Gfx,
        camera_x: f32,
        sub_line_offset_y: f32,
        min_y: usize,
        max_y: usize,
    ) {
        let lines = doc.lines();
        let highlighted_lines = doc.highlighted_lines();

        for (i, y) in (min_y..max_y).enumerate() {
            let line = &lines[y];
            let visual_y = Self::get_line_visual_y(i, sub_line_offset_y, gfx);

            if y >= highlighted_lines.len() {
                let visual_x = -camera_x;

                gfx.add_text(line.iter().copied(), visual_x, visual_y, &theme.normal);
            } else {
                let mut x = 0;
                let highlighted_line = &highlighted_lines[y];

                for highlight in highlighted_line.highlights() {
                    let visual_x = x as f32 * gfx.glyph_width() - camera_x;
                    let color = &theme.highlight_kind_to_color(highlight.kind);

                    x += gfx.add_text(
                        line[highlight.start..highlight.end].iter().copied(),
                        visual_x,
                        visual_y,
                        color,
                    );
                }
            }
        }
    }

    fn draw_cursors(
        &mut self,
        doc: &Doc,
        theme: &Theme,
        gfx: &mut Gfx,
        is_focused: bool,
        camera_x: f32,
        camera_y: f32,
        min_y: usize,
        max_y: usize,
    ) {
        for index in doc.cursor_indices() {
            let Some(selection) = doc.get_cursor(index).get_selection() else {
                continue;
            };

            let start = selection.start.max(Position::new(0, min_y as isize));
            let end = selection.end.min(Position::new(0, max_y as isize));
            let mut position = start;

            while position < end {
                let highlight_position = doc.position_to_visual(position, camera_x, camera_y, gfx);

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
            let cursor_width = (gfx.glyph_width() * 0.25).ceil();

            for index in doc.cursor_indices() {
                let cursor_position =
                    doc.position_to_visual(doc.get_cursor(index).position, 0.0, camera_y, gfx);

                gfx.add_rect(
                    Rect::new(
                        (cursor_position.x - cursor_width / 2.0).max(0.0) - camera_x,
                        cursor_position.y,
                        cursor_width,
                        gfx.line_height(),
                    ),
                    &theme.normal,
                );
            }
        }
    }
}
