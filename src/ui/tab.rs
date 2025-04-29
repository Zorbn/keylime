use core::f32;
use std::{iter::Enumerate, ops::Range};

use crate::{
    ctx::Ctx,
    digits::get_digits,
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
    core::{Ui, Widget},
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
    handled_cursor_position: Option<Position>,

    tab_bounds: Rect,
    gutter_bounds: Rect,
    doc_bounds: Rect,
}

impl Tab {
    pub fn new(data_index: usize) -> Self {
        Self {
            data_index,

            camera: Camera::new(),
            handled_cursor_position: None,

            tab_bounds: Rect::ZERO,
            gutter_bounds: Rect::ZERO,
            doc_bounds: Rect::ZERO,
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

    pub fn update(&mut self, widget: &Widget, ui: &mut Ui, doc: &mut Doc, ctx: &mut Ctx) {
        self.handled_cursor_position = Some(doc.get_cursor(CursorIndex::Main).position);

        let mut grapheme_handler = ui.get_grapheme_handler(widget, ctx.window);

        while let Some(grapheme) = grapheme_handler.next(ctx.window) {
            let mut text_buffer = ctx.buffers.text.take_mut();
            text_buffer.push_str(grapheme);

            handle_grapheme(&text_buffer, doc, ctx);

            ctx.buffers.text.replace(text_buffer);
        }

        let mut mousebind_handler = ui.get_mousebind_handler(widget, ctx.window);

        while let Some(mousebind) = mousebind_handler.next(ctx.window) {
            let visual_position = VisualPosition::new(mousebind.x, mousebind.y);

            if !self
                .doc_bounds
                .contains_position(VisualPosition::new(mousebind.x, mousebind.y))
            {
                mousebind_handler.unprocessed(ctx.window, mousebind);
                continue;
            }

            // The mouse position is shifted over by half
            // a glyph to make the cursor line up with the mouse.
            let visual_position = VisualPosition::new(
                visual_position.x + ctx.gfx.glyph_width() / 2.0,
                visual_position.y,
            )
            .unoffset_by(self.doc_bounds);

            let position = doc.visual_to_position(visual_position, self.camera.position(), ctx.gfx);

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: 0 | MOD_SHIFT,
                    kind,
                    is_drag,
                    ..
                } => {
                    handle_left_click(doc, position, mousebind.mods, kind, is_drag, ctx.gfx);
                    self.handled_cursor_position = Some(doc.get_cursor(CursorIndex::Main).position);
                }
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: MOD_CTRL | MOD_CMD,
                    is_drag: false,
                    ..
                } => {
                    doc.add_cursor(position, ctx.gfx);
                }
                _ => mousebind_handler.unprocessed(ctx.window, mousebind),
            }
        }

        let mut action_handler = ui.get_action_handler(widget, ctx.window);

        while let Some(action) = action_handler.next(ctx.window) {
            let was_handled = handle_action(action, self, doc, ctx);

            if !was_handled {
                action_handler.unprocessed(ctx.window, action);
            }
        }

        doc.combine_overlapping_cursors();
        doc.update_tokens();
    }

    pub fn update_camera(
        &mut self,
        widget: &Widget,
        ui: &mut Ui,
        doc: &Doc,
        ctx: &mut Ctx,
        dt: f32,
    ) {
        let mut mouse_scroll_handler = ui.get_mouse_scroll_handler(widget, ctx.window);

        while let Some(mouse_scroll) = mouse_scroll_handler.next(ctx.window) {
            let position = VisualPosition::new(mouse_scroll.x, mouse_scroll.y);

            if !self.doc_bounds.contains_position(position) {
                mouse_scroll_handler.unprocessed(ctx.window, mouse_scroll);
                continue;
            }

            let delta = mouse_scroll.delta * ctx.gfx.line_height();

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

        self.update_camera_vertical(doc, ctx.gfx, dt);
        self.update_camera_horizontal(doc, ctx.gfx, dt);
    }

    fn update_camera_vertical(&mut self, doc: &Doc, gfx: &mut Gfx, dt: f32) {
        let new_cursor_position = doc.get_cursor(CursorIndex::Main).position;
        let new_cursor_visual_position =
            doc.position_to_visual(new_cursor_position, self.camera.position(), gfx);

        let can_recenter = self.handled_cursor_position != Some(new_cursor_position);
        let recenter_distance = Self::get_doc_recenter_distance(doc);

        let target_y = new_cursor_visual_position.y + gfx.line_height() / 2.0;
        let max_y = (doc.lines().len() - 1) as f32 * gfx.line_height();

        let scroll_border_top = gfx.line_height() * recenter_distance as f32;
        let scroll_border_bottom = self.doc_bounds.height - scroll_border_top;

        self.camera.vertical.update(
            target_y,
            max_y,
            self.doc_bounds.height,
            scroll_border_top..=scroll_border_bottom,
            can_recenter,
            dt,
        );
    }

    fn update_camera_horizontal(&mut self, doc: &Doc, gfx: &mut Gfx, dt: f32) {
        let new_cursor_position = doc.get_cursor(CursorIndex::Main).position;
        let new_cursor_visual_position =
            doc.position_to_visual(new_cursor_position, self.camera.position(), gfx);

        let can_recenter = self.handled_cursor_position != Some(new_cursor_position);
        let recenter_distance = Self::get_doc_recenter_distance(doc);

        let target_x = new_cursor_visual_position.x + gfx.glyph_width() / 2.0;
        let max_x = f32::MAX;

        let scroll_border_left = gfx.glyph_width() * recenter_distance as f32;
        let scroll_border_right = self.doc_bounds.width - scroll_border_left;

        self.camera.horizontal.update(
            target_x,
            max_x,
            self.doc_bounds.height,
            scroll_border_left..=scroll_border_right,
            can_recenter,
            dt,
        );
    }

    fn get_doc_recenter_distance(doc: &Doc) -> usize {
        match doc.kind() {
            DocKind::Output => 1,
            _ => RECENTER_DISTANCE,
        }
    }

    pub fn tab_bounds(&self) -> Rect {
        self.tab_bounds
    }

    pub fn doc_bounds(&self) -> Rect {
        self.doc_bounds
    }

    pub fn doc_height_lines(&self, gfx: &Gfx) -> usize {
        (self.doc_bounds.height / gfx.line_height()) as usize
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
        ctx: &mut Ctx,
        is_focused: bool,
    ) {
        let language = ctx.config.get_language_for_doc(doc);

        if let Some(syntax) = language.and_then(|language| language.syntax.as_ref()) {
            doc.update_highlights(self.camera.position(), self.doc_bounds, syntax, ctx.gfx);
        }

        let camera_position = self.camera.position().floor();

        let min_y = (camera_position.y / ctx.gfx.line_height()) as usize;
        let sub_line_offset_y = camera_position.y - min_y as f32 * ctx.gfx.line_height();

        let max_y =
            ((camera_position.y + self.doc_bounds.height) / ctx.gfx.line_height()) as usize + 1;
        let max_y = max_y.min(doc.lines().len());

        let visible_lines = VisibleLines {
            offset: sub_line_offset_y,
            min_y,
            max_y,
        };

        if doc.kind() == DocKind::MultiLine {
            ctx.gfx.begin(Some(self.gutter_bounds));

            self.draw_gutter(doc, visible_lines, is_focused, ctx);

            ctx.gfx.end();
        }

        ctx.gfx.begin(Some(self.doc_bounds));

        if let Some(default_background) = default_background {
            ctx.gfx.add_rect(
                self.doc_bounds.unoffset_by(self.doc_bounds),
                default_background,
            );
        }

        self.draw_indent_guides(doc, camera_position, visible_lines, ctx);
        self.draw_lines(default_background, doc, camera_position, visible_lines, ctx);
        self.draw_diagnostics(doc, camera_position, visible_lines, ctx);
        self.draw_cursors(doc, is_focused, camera_position, visible_lines, ctx);
        self.draw_scroll_bar(doc, visible_lines, ctx);

        ctx.gfx.end();
    }

    fn draw_gutter(
        &mut self,
        doc: &Doc,
        visible_lines: VisibleLines,
        is_focused: bool,
        ctx: &mut Ctx,
    ) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        let cursor_y = doc.get_cursor(CursorIndex::Main).position.y;
        let mut digits = [0; 20];

        for (i, y) in visible_lines.enumerate() {
            let digits = get_digits(y + 1, &mut digits);
            let visual_y = Self::get_line_foreground_visual_y(i, visible_lines.offset, gfx);

            let width = digits.len() as f32 * gfx.glyph_width();
            let visual_x = self.gutter_bounds.width
                - width
                - (GUTTER_PADDING_WIDTH + GUTTER_BORDER_WIDTH) * gfx.glyph_width();

            let color = if is_focused && y == cursor_y {
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
        camera_position: VisualPosition,
        visible_lines: VisibleLines,
        ctx: &mut Ctx,
    ) {
        let language = ctx.config.get_language_for_doc(doc);
        let indent_width = language.map(|language| language.indent_width.measure(ctx.gfx));

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        let Some(indent_width) = indent_width else {
            return;
        };

        let mut indent_guide_x = 0;

        for (i, y) in visible_lines.enumerate() {
            let background_visual_y =
                Self::get_line_background_visual_y(i, visible_lines.offset, gfx);

            if !doc.is_line_whitespace(y) {
                indent_guide_x = doc.get_line_start(y)
            };

            for x in (indent_width..indent_guide_x).step_by(indent_width) {
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
        camera_position: VisualPosition,
        visible_lines: VisibleLines,
        ctx: &mut Ctx,
    ) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        let lines = doc.lines();
        let highlighted_lines = doc.highlighted_lines();

        for (i, y) in visible_lines.enumerate() {
            let line = &lines[y];
            let foreground_visual_y =
                Self::get_line_foreground_visual_y(i, visible_lines.offset, gfx);
            let background_visual_y =
                Self::get_line_background_visual_y(i, visible_lines.offset, gfx);

            let Some(highlights) = highlighted_lines
                .get(y)
                .map(|highlighted_line| highlighted_line.highlights())
                .filter(|highlights| !highlights.is_empty())
            else {
                let visual_x = -camera_position.x;

                gfx.add_text(&line[..], visual_x, foreground_visual_y, theme.normal);

                continue;
            };

            let mut x = 0.0;

            for highlight in highlights {
                let visual_x = x - camera_position.x;
                let foreground = ctx
                    .config
                    .theme
                    .highlight_kind_to_color(highlight.foreground);
                let highlighted_text = &line[highlight.start..highlight.end];

                if let Some(highlight_background) = highlight.background {
                    let background = ctx
                        .config
                        .theme
                        .highlight_kind_to_color(highlight_background);

                    if Some(background) != default_background {
                        gfx.add_background(
                            highlighted_text,
                            visual_x,
                            background_visual_y,
                            background,
                        );
                    }
                }

                x += gfx.add_text(highlighted_text, visual_x, foreground_visual_y, foreground);
            }
        }
    }

    fn draw_diagnostics(
        &mut self,
        doc: &Doc,
        camera_position: VisualPosition,
        visible_lines: VisibleLines,
        ctx: &mut Ctx,
    ) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        for language_server in ctx.lsp.iter_servers_mut() {
            for diagnostic in language_server.get_diagnostics_mut(doc) {
                if !diagnostic.is_visible() {
                    continue;
                }

                let color = diagnostic.color(theme);
                let (start, end) = diagnostic.range;

                if start == end && start.y >= visible_lines.min_y && start.y <= visible_lines.max_y
                {
                    let highlight_position = doc.position_to_visual(start, camera_position, gfx);

                    gfx.add_rect(
                        Rect::new(
                            highlight_position.x - gfx.glyph_width() / 2.0,
                            highlight_position.y + gfx.line_height() - gfx.border_width(),
                            gfx.glyph_width(),
                            gfx.border_width(),
                        ),
                        color,
                    );

                    continue;
                }

                let start = start.max(Position::new(0, visible_lines.min_y));
                let end = end.min(Position::new(0, visible_lines.max_y));
                let mut position = start;

                while position < end {
                    let highlight_position = doc.position_to_visual(position, camera_position, gfx);

                    let grapheme = doc.get_grapheme(position);
                    let grapheme_width = gfx.measure_text(grapheme);

                    gfx.add_rect(
                        Rect::new(
                            highlight_position.x,
                            highlight_position.y + gfx.line_height() - gfx.border_width(),
                            grapheme_width as f32 * gfx.glyph_width(),
                            gfx.border_width(),
                        ),
                        color,
                    );

                    position = doc.move_position(position, 1, 0, gfx);
                }
            }
        }
    }

    fn draw_cursors(
        &mut self,
        doc: &Doc,
        is_focused: bool,
        camera_position: VisualPosition,
        visible_lines: VisibleLines,
        ctx: &mut Ctx,
    ) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        for index in doc.cursor_indices() {
            let Some(selection) = doc.get_cursor(index).get_selection() else {
                continue;
            };

            let start = selection.start.max(Position::new(0, visible_lines.min_y));
            let end = selection.end.min(Position::new(0, visible_lines.max_y));
            let mut position = start;

            while position < end {
                let highlight_position = doc.position_to_visual(position, camera_position, gfx);

                let grapheme = doc.get_grapheme(position);
                let grapheme_width = gfx.measure_text(grapheme);

                gfx.add_rect(
                    Rect::new(
                        highlight_position.x,
                        highlight_position.y,
                        grapheme_width as f32 * gfx.glyph_width(),
                        gfx.line_height(),
                    ),
                    theme.selection,
                );

                position = doc.move_position(position, 1, 0, gfx);
            }
        }

        if is_focused && ctx.window.is_focused() {
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

    fn draw_scroll_bar(&mut self, doc: &Doc, visible_lines: VisibleLines, ctx: &mut Ctx) {
        if doc.kind() != DocKind::MultiLine {
            return;
        }

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        for language_server in ctx.lsp.iter_servers_mut() {
            for diagnostic in language_server.get_diagnostics_mut(doc) {
                if !diagnostic.is_visible() {
                    continue;
                }

                let color = diagnostic.color(theme);
                let (start, end) = diagnostic.range;

                gfx.add_rect(
                    self.doc_range_to_scrollbar_rect(start.y, end.y.max(start.y + 1), doc, gfx),
                    color,
                );
            }
        }

        for index in doc.cursor_indices() {
            let cursor_y = doc.get_cursor(index).position.y;

            gfx.add_rect(
                self.doc_range_to_scrollbar_rect(cursor_y, cursor_y + 1, doc, gfx),
                theme.normal,
            );
        }

        let doc_height_lines = self.doc_height_lines(gfx);

        gfx.add_rect(
            self.doc_range_to_scrollbar_rect(
                visible_lines.min_y,
                visible_lines.max_y.max(doc_height_lines),
                doc,
                gfx,
            ),
            theme.scroll_bar,
        );
    }

    fn doc_range_to_scrollbar_rect(
        &self,
        start_y: usize,
        end_y: usize,
        doc: &Doc,
        gfx: &Gfx,
    ) -> Rect {
        let doc_height_lines = self.doc_height_lines(gfx);
        let doc_len = doc.lines().len().max(doc_height_lines);

        let width = gfx.glyph_width() / 2.0;
        let x = self.doc_bounds.width - width;

        let start_y = (start_y as f32 / doc_len as f32) * self.doc_bounds.height;
        let end_y = ((end_y + 1) as f32 / doc_len as f32) * self.doc_bounds.height;

        let start_y = start_y.floor();
        let end_y = end_y.floor();

        Rect::new(x, start_y, width, (end_y - start_y).max(gfx.border_width()))
    }
}
