use core::f32;
use std::{iter::Enumerate, ops::Range};

use crate::{
    config::{language::Language, theme::Theme, Config},
    digits::get_digits,
    editor_buffers::EditorBuffers,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    input::{
        editing_actions::{handle_action, handle_grapheme, handle_left_click},
        keybind::{MOD_CMD, MOD_CTRL, MOD_SHIFT},
        mouse_button::MouseButton,
        mousebind::Mousebind,
    },
    platform::gfx::Gfx,
    text::{
        cursor_index::CursorIndex,
        doc::{Doc, DocKind},
    },
};

use super::{
    camera::{Camera, RECENTER_DISTANCE},
    color::Color,
    widget::WidgetHandle,
};

const GUTTER_PADDING_WIDTH: f32 = 1.0;
const GUTTER_BORDER_WIDTH: f32 = 0.5;

#[derive(Debug, Clone, Copy)]
struct VisibleLines {
    offset: f32,
    min_y: usize,
    max_y: usize,
}

impl VisibleLines {
    pub fn enumerate(&self) -> Enumerate<Range<usize>> {
        (self.min_y..self.max_y).enumerate()
    }
}

pub struct Tab {
    data_index: usize,

    pub camera: Camera,
    handled_cursor_position: Position,

    tab_bounds: Rect,
    gutter_bounds: Rect,
    doc_bounds: Rect,
}

impl Tab {
    pub fn new(data_index: usize) -> Self {
        Self {
            data_index,

            camera: Camera::new(),
            handled_cursor_position: Position::zero(),

            tab_bounds: Rect::zero(),
            gutter_bounds: Rect::zero(),
            doc_bounds: Rect::zero(),
        }
    }

    pub fn data_index(&self) -> usize {
        self.data_index
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
        widget: &mut WidgetHandle,
        doc: &mut Doc,
        buffers: &mut EditorBuffers,
        config: &Config,
        time: f32,
    ) {
        let language = config.get_language_for_doc(doc);

        self.handled_cursor_position = doc.get_cursor(CursorIndex::Main).position;

        let mut grapheme_handler = widget.get_grapheme_handler();

        while let Some(grapheme) = grapheme_handler.next(widget.window()) {
            handle_grapheme(grapheme, doc, &mut buffers.lines, time);
        }

        let mut mousebind_handler = widget.get_mousebind_handler();

        while let Some(mousebind) = mousebind_handler.next(widget.window()) {
            let visual_position = VisualPosition::new(mousebind.x, mousebind.y);

            if !self
                .doc_bounds
                .contains_position(VisualPosition::new(mousebind.x, mousebind.y))
            {
                mousebind_handler.unprocessed(widget.window(), mousebind);
                continue;
            }

            // The mouse position is shifted over by half
            // a glyph to make the cursor line up with the mouse.
            let visual_position = VisualPosition::new(
                visual_position.x + widget.gfx().glyph_width() / 2.0,
                visual_position.y,
            )
            .unoffset_by(self.doc_bounds);

            let position =
                doc.visual_to_position(visual_position, self.camera.position(), widget.gfx());

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: 0 | MOD_SHIFT,
                    kind,
                    is_drag,
                    ..
                } => {
                    handle_left_click(doc, position, mousebind.mods, kind, is_drag);
                    self.handled_cursor_position = doc.get_cursor(CursorIndex::Main).position;
                }
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: MOD_CTRL | MOD_CMD,
                    is_drag: false,
                    ..
                } => {
                    doc.add_cursor(position);
                }
                _ => mousebind_handler.unprocessed(widget.window(), mousebind),
            }
        }

        let mut action_handler = widget.get_action_handler();

        while let Some(action) = action_handler.next(widget.window()) {
            let was_handled = handle_action(action, widget.window(), doc, language, buffers, time);

            if !was_handled {
                action_handler.unprocessed(widget.window(), action);
            }
        }

        doc.combine_overlapping_cursors();
        doc.update_tokens();
    }

    pub fn update_camera(&mut self, widget: &mut WidgetHandle, doc: &Doc, dt: f32) {
        let mut mouse_scroll_handler = widget.get_mouse_scroll_handler();

        while let Some(mouse_scroll) = mouse_scroll_handler.next(widget.window()) {
            let position = VisualPosition::new(mouse_scroll.x, mouse_scroll.y);

            if !self.doc_bounds.contains_position(position) {
                mouse_scroll_handler.unprocessed(widget.window(), mouse_scroll);
                continue;
            }

            let delta = mouse_scroll.delta * widget.gfx().line_height();

            if mouse_scroll.is_horizontal {
                self.camera.vertical.reset_velocity();
                self.camera
                    .horizontal
                    .scroll(-delta, mouse_scroll.is_precise);
            } else {
                self.camera.horizontal.reset_velocity();
                self.camera.vertical.scroll(delta, mouse_scroll.is_precise);
            }
        }

        let gfx = widget.gfx();

        self.update_camera_vertical(doc, gfx, dt);
        self.update_camera_horizontal(doc, gfx, dt);
    }

    fn update_camera_vertical(&mut self, doc: &Doc, gfx: &Gfx, dt: f32) {
        let new_cursor_position = doc.get_cursor(CursorIndex::Main).position;
        let new_cursor_visual_position =
            doc.position_to_visual(new_cursor_position, self.camera.position(), gfx);

        let can_recenter = self.handled_cursor_position != new_cursor_position;

        let target_y = new_cursor_visual_position.y + gfx.line_height() / 2.0;
        let max_y = (doc.lines().len() - 1) as f32 * gfx.line_height();

        let scroll_border_top = gfx.line_height() * RECENTER_DISTANCE as f32;
        let scroll_border_bottom = self.doc_bounds.height - scroll_border_top - gfx.line_height();

        self.camera.vertical.update(
            target_y,
            max_y,
            self.doc_bounds.height,
            scroll_border_top..=scroll_border_bottom,
            can_recenter,
            dt,
        );
    }

    fn update_camera_horizontal(&mut self, doc: &Doc, gfx: &Gfx, dt: f32) {
        let new_cursor_position = doc.get_cursor(CursorIndex::Main).position;
        let new_cursor_visual_position =
            doc.position_to_visual(new_cursor_position, self.camera.position(), gfx);

        let can_recenter = self.handled_cursor_position != new_cursor_position;

        let target_x = new_cursor_visual_position.x + gfx.glyph_width() / 2.0;
        let max_x = f32::MAX;

        let scroll_border_left = gfx.glyph_width() * RECENTER_DISTANCE as f32;
        let scroll_border_right = self.doc_bounds.width - scroll_border_left - gfx.glyph_width();

        self.camera.horizontal.update(
            target_x,
            max_x,
            self.doc_bounds.height,
            scroll_border_left..=scroll_border_right,
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

    fn get_line_foreground_visual_y(index: usize, sub_line_offset_y: f32, gfx: &Gfx) -> f32 {
        Self::get_line_background_visual_y(index, sub_line_offset_y, gfx) + gfx.line_padding()
    }

    fn get_line_background_visual_y(index: usize, sub_line_offset_y: f32, gfx: &Gfx) -> f32 {
        index as f32 * gfx.line_height() - sub_line_offset_y
    }

    pub fn draw(
        &mut self,
        default_background: Option<Color>,
        doc: &mut Doc,
        config: &Config,
        gfx: &mut Gfx,
        is_focused: bool,
    ) {
        let language = config.get_language_for_doc(doc);

        if let Some(syntax) = language.and_then(|language| language.syntax.as_ref()) {
            doc.update_highlights(self.camera.position(), self.doc_bounds, syntax, gfx);
        }

        let camera_position = self.camera.position().floor();

        let min_y = (camera_position.y / gfx.line_height()) as usize;
        let sub_line_offset_y = camera_position.y - min_y as f32 * gfx.line_height();

        let max_y = ((camera_position.y + self.doc_bounds.height) / gfx.line_height()) as usize + 1;
        let max_y = max_y.min(doc.lines().len());

        let visible_lines = VisibleLines {
            offset: sub_line_offset_y,
            min_y,
            max_y,
        };

        if doc.kind() == DocKind::MultiLine {
            gfx.begin(Some(self.gutter_bounds));

            self.draw_gutter(doc, &config.theme, gfx, visible_lines, is_focused);

            gfx.end();
        }

        gfx.begin(Some(self.doc_bounds));

        if let Some(default_background) = default_background {
            gfx.add_rect(
                self.doc_bounds.unoffset_by(self.doc_bounds),
                default_background,
            );
        }

        self.draw_indent_guides(
            doc,
            language,
            &config.theme,
            gfx,
            camera_position,
            visible_lines,
        );
        self.draw_lines(
            default_background,
            doc,
            &config.theme,
            gfx,
            camera_position,
            visible_lines,
        );
        self.draw_cursors(
            doc,
            &config.theme,
            gfx,
            is_focused,
            camera_position,
            visible_lines,
        );

        gfx.end();
    }

    fn draw_gutter(
        &mut self,
        doc: &Doc,
        theme: &Theme,
        gfx: &mut Gfx,
        visible_lines: VisibleLines,
        is_focused: bool,
    ) {
        let cursor_y = doc.get_cursor(CursorIndex::Main).position.y;

        let mut digits = [' '; 20];

        for (i, y) in visible_lines.enumerate() {
            let digits = get_digits(y + 1, &mut digits);
            let visual_y = Self::get_line_foreground_visual_y(i, visible_lines.offset, gfx);

            let width = digits.len() as f32 * gfx.glyph_width();
            let visual_x = self.gutter_bounds.width
                - width
                - (GUTTER_PADDING_WIDTH + GUTTER_BORDER_WIDTH) * gfx.glyph_width();

            let color = if is_focused && y as isize == cursor_y {
                theme.normal
            } else {
                theme.line_number
            };

            gfx.add_text(digits, visual_x, visual_y, color);
        }

        gfx.add_rect(
            self.gutter_bounds
                .unoffset_by(self.gutter_bounds)
                .right_border(gfx.border_width()),
            theme.border,
        );
    }

    fn draw_indent_guides(
        &mut self,
        doc: &Doc,
        language: Option<&Language>,
        theme: &Theme,
        gfx: &mut Gfx,
        camera_position: VisualPosition,
        visible_lines: VisibleLines,
    ) {
        let indent_width =
            language.map(|language| language.indent_width.chars().map(Gfx::get_char_width).sum());

        let Some(indent_width) = indent_width else {
            return;
        };

        let mut indent_guide_x = 0;

        for (i, y) in visible_lines.enumerate() {
            let background_visual_y =
                Self::get_line_background_visual_y(i, visible_lines.offset, gfx);

            if !doc.is_line_whitespace(y as isize) {
                indent_guide_x = doc.get_line_start(y as isize)
            };

            for x in (indent_width..indent_guide_x).step_by(indent_width as usize) {
                let visual_x = gfx.glyph_width() * x as f32 - camera_position.x;

                gfx.add_rect(
                    Rect::new(
                        visual_x,
                        background_visual_y,
                        gfx.border_width(),
                        gfx.line_height(),
                    ),
                    theme.border,
                );
            }
        }
    }

    fn draw_lines(
        &mut self,
        default_background: Option<Color>,
        doc: &Doc,
        theme: &Theme,
        gfx: &mut Gfx,
        camera_position: VisualPosition,
        visible_lines: VisibleLines,
    ) {
        let lines = doc.lines();
        let highlighted_lines = doc.highlighted_lines();

        for (i, y) in visible_lines.enumerate() {
            let line = &lines[y];
            let foreground_visual_y =
                Self::get_line_foreground_visual_y(i, visible_lines.offset, gfx);
            let background_visual_y =
                Self::get_line_background_visual_y(i, visible_lines.offset, gfx);

            if y >= highlighted_lines.len() {
                let visual_x = -camera_position.x;

                gfx.add_text(
                    line[..].chars(),
                    visual_x,
                    foreground_visual_y,
                    theme.normal,
                );

                continue;
            }

            let mut x = 0;
            let highlighted_line = &highlighted_lines[y];

            for highlight in highlighted_line.highlights() {
                let visual_x = x as f32 * gfx.glyph_width() - camera_position.x;
                let foreground = theme.highlight_kind_to_color(highlight.foreground);

                if let Some(highlight_background) = highlight.background {
                    let background = theme.highlight_kind_to_color(highlight_background);

                    Self::draw_background(
                        highlight.end - highlight.start,
                        gfx,
                        visual_x,
                        background_visual_y,
                        default_background,
                        background,
                    );
                }

                x += gfx.add_text(
                    line[highlight.start..highlight.end].chars(),
                    visual_x,
                    foreground_visual_y,
                    foreground,
                );
            }
        }
    }

    fn draw_background(
        len: isize,
        gfx: &mut Gfx,
        x: f32,
        y: f32,
        default_background: Option<Color>,
        color: Color,
    ) {
        if Some(color) == default_background {
            return;
        }

        for i in 0..len {
            gfx.add_rect(
                Rect::new(
                    x + i as f32 * gfx.glyph_width(),
                    y,
                    gfx.glyph_width(),
                    gfx.line_height(),
                ),
                color,
            );
        }
    }

    fn draw_cursors(
        &mut self,
        doc: &Doc,
        theme: &Theme,
        gfx: &mut Gfx,
        is_focused: bool,
        camera_position: VisualPosition,
        visible_lines: VisibleLines,
    ) {
        for index in doc.cursor_indices() {
            let Some(selection) = doc.get_cursor(index).get_selection() else {
                continue;
            };

            let start = selection
                .start
                .max(Position::new(0, visible_lines.min_y as isize));
            let end = selection
                .end
                .min(Position::new(0, visible_lines.max_y as isize));
            let mut position = start;

            while position < end {
                let highlight_position = doc.position_to_visual(position, camera_position, gfx);

                let char = doc.get_grapheme(position);
                let char_width = Gfx::measure_text(char);

                gfx.add_rect(
                    Rect::new(
                        highlight_position.x,
                        highlight_position.y,
                        char_width as f32 * gfx.glyph_width(),
                        gfx.line_height(),
                    ),
                    theme.selection,
                );

                position = doc.move_position(position, Position::new(1, 0));
            }
        }

        if is_focused {
            let cursor_width = (gfx.glyph_width() * 0.25).ceil();

            for index in doc.cursor_indices() {
                let cursor_position = doc.position_to_visual(
                    doc.get_cursor(index).position,
                    VisualPosition::new(0.0, camera_position.y),
                    gfx,
                );

                gfx.add_rect(
                    Rect::new(
                        (cursor_position.x - cursor_width / 2.0).max(0.0) - camera_position.x,
                        cursor_position.y,
                        cursor_width,
                        gfx.line_height(),
                    ),
                    theme.normal,
                );
            }
        }
    }
}
