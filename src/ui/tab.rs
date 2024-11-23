use core::f32;

use crate::{
    config::{theme::Theme, Config},
    digits::get_digits,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    input::{
        editing_actions::{handle_char, handle_keybind},
        keybind::{MOD_CTRL, MOD_SHIFT},
        mouse_button::MouseButton,
        mousebind::Mousebind,
    },
    platform::{gfx::Gfx, window::Window},
    temp_buffer::TempBuffer,
    text::{
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

        while let Some(c) = char_handler.next(window) {
            handle_char(c, doc, line_pool, time);
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
            let was_handled =
                handle_keybind(keybind, window, doc, language, line_pool, text_buffer, time);

            if !was_handled {
                keybind_handler.unprocessed(window, keybind);
            }
        }

        doc.combine_overlapping_cursors();
        self.update_camera_vertical(doc, window, handled_cursor_position, dt);
        self.update_camera_horizontal(doc, window, handled_cursor_position, dt);
    }

    fn update_camera_vertical(
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

    fn update_camera_horizontal(
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

            self.draw_gutter(
                doc,
                &config.theme,
                gfx,
                sub_line_offset_y,
                min_y,
                max_y,
                is_focused,
            );

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
        is_focused: bool,
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

            let color = if is_focused && y as isize == cursor_y {
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
