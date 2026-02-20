use core::f32;
use std::{iter::Enumerate, ops::Range};

use crate::{
    config::language::{DelimiterKind, Language},
    ctx::Ctx,
    geometry::{
        easing::ease_out_quart, position::Position, quad::Quad, rect::Rect,
        visual_position::VisualPosition,
    },
    input::{
        editing_actions::{handle_action, handle_grapheme, handle_left_click},
        mods::{Mod, Mods},
        mouse_button::MouseButton,
        mousebind::{MouseClickCount, Mousebind, MousebindKind},
    },
    lsp::types::DecodedRange,
    platform::gfx::Gfx,
    pool::{format_pooled, Pooled},
    text::{
        cursor_index::CursorIndex,
        doc::{Doc, DocFlag},
        grapheme_category::GraphemeCategory,
        selection::Selection,
        syntax_highlighter::HighlightedLine,
    },
    ui::{
        camera::CameraRecenterRequest,
        core::{Ui, WidgetSettings},
    },
};

use super::{
    camera::{Camera, RECENTER_DISTANCE},
    color::Color,
    core::WidgetId,
    slot_list::SlotId,
};

const GUTTER_PADDING_WIDTH: f32 = 1.0;
const GUTTER_BORDER_WIDTH: f32 = 0.5;

const CURSOR_ANIMATION_SPEED: f64 = 8.0;
const TAB_ANIMATION_SPEED: f32 = 10.0;

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

struct CursorAnimationState {
    last_time: f64,
    last_position: VisualPosition,
    position: VisualPosition,
}

struct TabAnimationState {
    x: f32,
}

pub struct Tab {
    widget_id: WidgetId,
    data_id: SlotId,

    pub camera: Camera,
    handled_cursor_position: Position,
    mouse_drag: Option<MouseClickCount>,
    handled_doc_len: usize,
    cursor_animation_states: Vec<CursorAnimationState>,

    tab_animation_state: TabAnimationState,
    tab_bounds: Rect,
    gutter_bounds: Rect,
    margin: f32,
}

impl Tab {
    pub fn new(parent_id: WidgetId, data_id: SlotId, ui: &mut Ui) -> Self {
        Self {
            widget_id: ui.new_widget(parent_id, WidgetSettings::default()),
            data_id,

            camera: Camera::new(),
            handled_cursor_position: Position::ZERO,
            mouse_drag: None,
            handled_doc_len: 1,
            cursor_animation_states: Vec::new(),

            tab_animation_state: TabAnimationState { x: 0.0 },
            tab_bounds: Rect::ZERO,
            gutter_bounds: Rect::ZERO,
            margin: 0.0,
        }
    }

    pub fn data_id(&self) -> SlotId {
        self.data_id
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.camera.is_moving()
            || (self.tab_bounds.x - self.tab_animation_state.x).abs() > 0.5
            || self.cursor_animation_states.iter().any(|animation_state| {
                self.cursor_animation_progress(ctx.time, animation_state.last_time) < 1.0
            })
    }

    // pub fn layout(
    //     &mut self,
    //     tab_bounds: Rect,
    //     doc_bounds: Rect,
    //     margin: f32,
    //     doc: &Doc,
    //     gfx: &Gfx,
    // ) {
    //     if self.tab_bounds == Rect::ZERO {
    //         self.tab_animation_state.x = tab_bounds.x;
    //     }

    //     self.tab_bounds = tab_bounds;

    //     let gutter_width = if doc.flags().contains(DocFlag::ShowGutter) {
    //         let max_gutter_digits = (doc.lines().len() as f32).log10().floor() + 1.0;

    //         (max_gutter_digits + GUTTER_PADDING_WIDTH * 2.0 + GUTTER_BORDER_WIDTH)
    //             * gfx.glyph_width()
    //     } else {
    //         0.0
    //     };

    //     self.gutter_bounds = Rect::new(doc_bounds.x, doc_bounds.y, gutter_width, doc_bounds.height);
    //     self.doc_bounds = doc_bounds.shrink_left_by(self.gutter_bounds);
    //     self.margin = margin;
    // }

    pub fn update(&mut self, doc: &mut Doc, ctx: &mut Ctx) {
        let mut grapheme_handler = ctx.ui.grapheme_handler(self.widget_id, ctx.window);

        while let Some(grapheme) = grapheme_handler.next(ctx.window) {
            let grapheme: Pooled<String> = grapheme.into();

            handle_grapheme(&grapheme, doc, ctx);
        }

        let mut global_mousebind_handler = ctx.window.mousebind_handler();

        while let Some(mousebind) = global_mousebind_handler.next(ctx.window) {
            if let Mousebind {
                button: Some(MouseButton::Left),
                kind: MousebindKind::Release,
                ..
            } = mousebind
            {
                self.mouse_drag = None;
            }

            global_mousebind_handler.unprocessed(ctx.window, mousebind);
        }

        let mut mousebind_handler = ctx.ui.mousebind_handler(self.widget_id, ctx.window);

        while let Some(mousebind) = mousebind_handler.next(ctx.window) {
            if !ctx
                .ui
                .bounds(self.widget_id)
                .contains_position(VisualPosition::new(mousebind.x, mousebind.y))
            {
                mousebind_handler.unprocessed(ctx.window, mousebind);
                continue;
            }

            let position = self.mouse_to_position(mousebind.x, mousebind.y, doc, ctx.ui, ctx.gfx);

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: Mods::NONE | Mods::SHIFT,
                    count,
                    kind: MousebindKind::Press,
                    ..
                } => {
                    handle_left_click(doc, position, mousebind.mods, count, false, ctx.gfx);

                    self.handled_cursor_position = doc.cursor(CursorIndex::Main).position;
                    self.mouse_drag = Some(count);
                }
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods,
                    kind: MousebindKind::Press,
                    ..
                } if mods.contains(Mod::Ctrl) || mods.contains(Mod::Cmd) => {
                    if mods.contains(Mod::Alt) {
                        doc.add_cursor_at(position, ctx.gfx);
                    } else {
                        doc.lsp_definition(position, ctx);
                    }
                }
                _ => mousebind_handler.unprocessed(ctx.window, mousebind),
            }
        }

        if let Some(count) = self.mouse_drag {
            let visual_position = ctx.window.mouse_position();
            let position =
                self.mouse_to_position(visual_position.x, visual_position.y, doc, ctx.ui, ctx.gfx);

            handle_left_click(doc, position, Mods::NONE, count, true, ctx.gfx);

            self.handled_cursor_position = doc.cursor(CursorIndex::Main).position;
        }

        let mut action_handler = ctx.ui.action_handler(self.widget_id, ctx.window);

        while let Some(action) = action_handler.next(ctx) {
            let was_handled = handle_action(action, self, doc, ctx);

            if !was_handled {
                action_handler.unprocessed(ctx.window, action);
            }
        }

        doc.combine_overlapping_cursors();
        doc.update_tokens();
    }

    fn mouse_to_position(&self, x: f32, y: f32, doc: &Doc, ui: &Ui, gfx: &mut Gfx) -> Position {
        // Offset the raw mouse position to make selecting between characters more natural.
        let visual_position = VisualPosition::new(x + 0.25 * gfx.glyph_width(), y);

        self.visual_to_position(visual_position, doc, ui, gfx)
    }

    pub fn skip_cursor_animations(&mut self, doc: &Doc, ctx: &mut Ctx) {
        self.cursor_animation_states.clear();

        for index in doc.cursor_indices() {
            let cursor = doc.cursor(index);
            let cursor_position =
                self.position_to_visual(cursor.position, VisualPosition::ZERO, doc, ctx.gfx);

            self.cursor_animation_states.push(CursorAnimationState {
                last_time: ctx.time,
                last_position: cursor_position,
                position: cursor_position,
            });
        }
    }

    pub fn animate(&mut self, widget_id: Option<WidgetId>, doc: &Doc, ctx: &mut Ctx, dt: f32) {
        self.animate_tab(dt);
        self.animate_cursors(doc, ctx);
        self.animate_camera(widget_id, doc, ctx, dt);
    }

    fn animate_tab(&mut self, dt: f32) {
        self.tab_animation_state.x +=
            (self.tab_bounds.x - self.tab_animation_state.x) * TAB_ANIMATION_SPEED * dt;
    }

    fn animate_cursors(&mut self, doc: &Doc, ctx: &mut Ctx) {
        let gfx = &mut ctx.gfx;

        self.cursor_animation_states.truncate(doc.cursors_len());

        for (i, index) in doc.cursor_indices().enumerate() {
            let cursor = doc.cursor(index);
            let cursor_position =
                self.position_to_visual(cursor.position, VisualPosition::ZERO, doc, gfx);

            if i >= self.cursor_animation_states.len() {
                self.cursor_animation_states.push(CursorAnimationState {
                    last_time: ctx.time,
                    last_position: cursor_position,
                    position: cursor_position,
                });

                continue;
            }

            let animation_state = &mut self.cursor_animation_states[i];

            if animation_state.position != cursor_position {
                animation_state.last_position = animation_state.position;
                animation_state.position = cursor_position;
                animation_state.last_time = ctx.time;
            }
        }
    }

    fn animate_camera(&mut self, widget_id: Option<WidgetId>, doc: &Doc, ctx: &mut Ctx, dt: f32) {
        if let Some(widget_id) = widget_id {
            self.handle_mouse_scrolls(widget_id, ctx);
        }

        self.animate_camera_vertical(doc, ctx, dt);
        self.animate_camera_horizontal(doc, ctx, dt);

        self.handled_cursor_position = doc.cursor(CursorIndex::Main).position;
        self.handled_doc_len = doc.lines().len();
    }

    fn handle_mouse_scrolls(&mut self, widget_id: WidgetId, ctx: &mut Ctx) {
        let mut mouse_scroll_handler = ctx.ui.mouse_scroll_handler(widget_id, ctx.window);

        while let Some(mouse_scroll) = mouse_scroll_handler.next(ctx.window) {
            let position = VisualPosition::new(mouse_scroll.x, mouse_scroll.y);

            if !ctx.ui.bounds(self.widget_id).contains_position(position) {
                mouse_scroll_handler.unprocessed(ctx.window, mouse_scroll);
                continue;
            }

            let delta = mouse_scroll.delta * ctx.gfx.line_height();

            if mouse_scroll.is_horizontal {
                self.camera.vertical.reset_velocity();
                self.camera.horizontal.scroll(-delta, mouse_scroll.kind);
            } else {
                self.camera.horizontal.reset_velocity();
                self.camera.vertical.scroll(delta, mouse_scroll.kind);
            }
        }
    }

    pub fn skip_camera_animations(&mut self, doc: &Doc, ctx: &mut Ctx) {
        let recenter_request = self.recenter_request_vertical(doc, ctx);
        let max_y = self.camera_max_y(doc, ctx.ui, ctx.gfx);
        let bounds = ctx.ui.bounds(self.widget_id);

        self.camera
            .vertical
            .skip_animation(recenter_request, max_y, bounds.height);

        let recenter_request = self.recenter_request_horizontal(doc, ctx);

        self.camera
            .horizontal
            .skip_animation(recenter_request, f32::MAX, bounds.width);
    }

    fn animate_camera_vertical(&mut self, doc: &Doc, ctx: &mut Ctx, dt: f32) {
        let recenter_request = self.recenter_request_vertical(doc, ctx);
        let max_y = self.camera_max_y(doc, ctx.ui, ctx.gfx);
        let bounds = ctx.ui.bounds(self.widget_id);

        self.camera
            .vertical
            .animate(recenter_request, max_y, bounds.height, dt);
    }

    fn camera_max_y(&self, doc: &Doc, ui: &Ui, gfx: &Gfx) -> f32 {
        let last_line_y = self.last_line_y(doc, gfx);

        if doc.flags().contains(DocFlag::AllowScrollingPastBottom) {
            last_line_y
        } else {
            (last_line_y - self.doc_bounds(ui).height + gfx.line_height()).max(0.0)
        }
    }

    fn last_line_y(&self, doc: &Doc, gfx: &Gfx) -> f32 {
        let doc_len = doc.lines().len();

        (doc_len - 1) as f32 * gfx.line_height() + self.margin * 2.0
    }

    fn recenter_request_vertical(&self, doc: &Doc, ctx: &mut Ctx) -> CameraRecenterRequest {
        if self.mouse_drag.is_some() {
            self.recenter_request_dragging_vertical(ctx)
        } else if doc.flags().contains(DocFlag::RecenterOnBottom) {
            self.recenter_request_on_bottom_vertical(doc, ctx)
        } else {
            self.recenter_request_on_cursor_vertical(doc, ctx)
        }
    }

    fn recenter_request_dragging_vertical(&self, ctx: &Ctx) -> CameraRecenterRequest {
        let bounds = ctx.ui.bounds(self.widget_id);
        let mouse_position = ctx.window.mouse_position().unoffset_by(bounds);

        CameraRecenterRequest {
            can_start: true,
            target_position: mouse_position.y,
            scroll_border: 0.0,
        }
    }

    fn recenter_request_on_bottom_vertical(&self, doc: &Doc, ctx: &Ctx) -> CameraRecenterRequest {
        let gfx = &ctx.gfx;

        let doc_len = doc.lines().len();
        let last_line_y = self.last_line_y(doc, gfx);

        CameraRecenterRequest {
            can_start: self.handled_doc_len != doc_len,
            target_position: last_line_y - self.camera.y(),
            scroll_border: gfx.line_height(),
        }
    }

    fn recenter_request_on_cursor_vertical(
        &self,
        doc: &Doc,
        ctx: &mut Ctx,
    ) -> CameraRecenterRequest {
        let gfx = &mut ctx.gfx;

        let new_cursor_position = doc.cursor(CursorIndex::Main).position;
        let new_cursor_visual_position =
            self.position_to_visual(new_cursor_position, self.camera.position(), doc, gfx);

        CameraRecenterRequest {
            can_start: self.handled_cursor_position != new_cursor_position,
            target_position: new_cursor_visual_position.y + gfx.line_height() / 2.0,
            scroll_border: gfx.line_height() * RECENTER_DISTANCE as f32,
        }
    }

    fn animate_camera_horizontal(&mut self, doc: &Doc, ctx: &mut Ctx, dt: f32) {
        let recenter_request = self.recenter_request_horizontal(doc, ctx);
        let bounds = ctx.ui.bounds(self.widget_id);

        self.camera
            .horizontal
            .animate(recenter_request, f32::MAX, bounds.width, dt);
    }

    fn recenter_request_horizontal(&self, doc: &Doc, ctx: &mut Ctx) -> CameraRecenterRequest {
        if self.mouse_drag.is_some() {
            self.recenter_request_dragging_horizontal(ctx)
        } else {
            self.recenter_request_on_cursor_horizontal(doc, ctx)
        }
    }

    fn recenter_request_dragging_horizontal(&self, ctx: &Ctx) -> CameraRecenterRequest {
        let bounds = ctx.ui.bounds(self.widget_id);
        let mouse_position = ctx.window.mouse_position().unoffset_by(bounds);

        CameraRecenterRequest {
            can_start: true,
            target_position: mouse_position.x,
            scroll_border: 0.0,
        }
    }

    fn recenter_request_on_cursor_horizontal(
        &self,
        doc: &Doc,
        ctx: &mut Ctx,
    ) -> CameraRecenterRequest {
        let gfx = &mut ctx.gfx;

        let new_cursor_position = doc.cursor(CursorIndex::Main).position;
        let new_cursor_visual_position =
            self.position_to_visual(new_cursor_position, self.camera.position(), doc, gfx);

        CameraRecenterRequest {
            can_start: self.handled_cursor_position != new_cursor_position,
            target_position: new_cursor_visual_position.x + gfx.glyph_width() / 2.0,
            scroll_border: gfx.glyph_width() * RECENTER_DISTANCE as f32,
        }
    }

    pub fn visual_to_position(
        &self,
        visual: VisualPosition,
        doc: &Doc,
        ui: &Ui,
        gfx: &mut Gfx,
    ) -> Position {
        let visual = self.visual_position_in_doc(visual, ui);
        doc.visual_to_position(visual, self.camera.position(), gfx)
    }

    pub fn visual_to_position_unclamped(
        &self,
        visual: VisualPosition,
        doc: &Doc,
        ui: &Ui,
        gfx: &mut Gfx,
    ) -> Option<Position> {
        if !ui.bounds(self.widget_id).contains_position(visual) {
            return None;
        }

        let visual = self.visual_position_in_doc(visual, ui);
        doc.visual_to_position_unclamped(visual, self.camera.position(), gfx)
    }

    fn position_to_visual(
        &self,
        position: Position,
        camera_position: VisualPosition,
        doc: &Doc,
        gfx: &mut Gfx,
    ) -> VisualPosition {
        let visual = doc.position_to_visual(position, camera_position, gfx);

        self.visual_position_in_tab(visual)
    }

    fn visual_position_in_doc(&self, visual: VisualPosition, ui: &Ui) -> VisualPosition {
        let bounds = ui.bounds(self.widget_id);
        let visual = visual.unoffset_by(bounds);

        VisualPosition::new(visual.x - self.margin, visual.y - self.margin)
    }

    fn visual_position_in_tab(&self, visual: VisualPosition) -> VisualPosition {
        VisualPosition::new(visual.x + self.margin, visual.y + self.margin)
    }

    pub fn set_tab_animation_x(&mut self, x: f32) {
        self.tab_animation_state.x = x;
    }

    pub fn visual_tab_bounds(&self) -> Rect {
        Rect {
            x: self.tab_animation_state.x,
            ..self.tab_bounds
        }
    }

    pub fn tab_bounds(&self) -> Rect {
        self.tab_bounds
    }

    // TODO:
    pub fn doc_bounds(&self, ui: &Ui) -> Rect {
        ui.bounds(self.widget_id)
    }

    pub fn doc_height_lines(&self, ui: &Ui, gfx: &Gfx) -> usize {
        (ui.bounds(self.widget_id).height / gfx.line_height()) as usize
    }

    pub fn cursor_width(gfx: &Gfx) -> f32 {
        gfx.border_width() * 2.0
    }

    fn line_foreground_visual_y(&self, index: usize, sub_line_offset_y: f32, gfx: &Gfx) -> f32 {
        self.line_background_visual_y(index, sub_line_offset_y, gfx) + gfx.line_padding_y()
    }

    fn line_background_visual_y(&self, index: usize, sub_line_offset_y: f32, gfx: &Gfx) -> f32 {
        index as f32 * gfx.line_height() - sub_line_offset_y + self.margin
    }

    pub fn update_highlights(&self, language: &Language, doc: &mut Doc, ctx: &mut Ctx) {
        if let Some(syntax) = language.syntax.as_ref() {
            let bounds = ctx.ui.bounds(self.widget_id);
            doc.update_highlights(self.camera.position(), bounds, syntax, ctx.gfx);
        }
    }

    pub fn draw(
        &self,
        colors @ (_, background): (Option<Color>, Option<Color>),
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) {
        let language = ctx.config.get_language_for_doc(doc);

        if let Some(language) = language {
            self.update_highlights(language, doc, ctx);
        }

        let bounds = ctx.ui.bounds(self.widget_id);
        let camera_position = self.camera.position().floor();

        let min_y = (camera_position.y / ctx.gfx.line_height()) as usize;
        let sub_line_offset_y = camera_position.y - min_y as f32 * ctx.gfx.line_height();

        let max_y = ((camera_position.y + bounds.height) / ctx.gfx.line_height()) as usize + 1;
        let max_y = max_y.min(doc.lines().len());

        let visible_lines = VisibleLines {
            offset: sub_line_offset_y,
            min_y,
            max_y,
        };

        if doc.flags().contains(DocFlag::ShowGutter) {
            ctx.gfx.begin(Some(self.gutter_bounds));

            self.draw_gutter(doc, visible_lines, ctx);

            ctx.gfx.end();
        }

        ctx.gfx.begin(Some(bounds));

        if let Some(background) = background {
            ctx.gfx.add_rect(bounds.unoffset_by(bounds), background);
        }

        self.draw_indent_guides(doc, camera_position, visible_lines, ctx);
        self.draw_lines(colors, doc, camera_position, visible_lines, ctx);
        self.draw_diagnostics(doc, camera_position, visible_lines, ctx);
        self.draw_go_to_definition_hint(doc, camera_position, ctx);
        self.draw_cursors(doc, camera_position, visible_lines, ctx);
        self.draw_scroll_bar(doc, camera_position, ctx);

        ctx.gfx.end();
    }

    fn draw_gutter(&self, doc: &Doc, visible_lines: VisibleLines, ctx: &mut Ctx) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        let cursor_y = doc.cursor(CursorIndex::Main).position.y;

        for (i, y) in visible_lines.enumerate() {
            let line_number = format_pooled!("{}", y + 1);
            let visual_y = self.line_foreground_visual_y(i, visible_lines.offset, gfx);

            let width = line_number.len() as f32 * gfx.glyph_width();
            let visual_x = self.gutter_bounds.width
                - width
                - (GUTTER_PADDING_WIDTH + GUTTER_BORDER_WIDTH) * gfx.glyph_width();

            let color = if y == cursor_y {
                theme.normal
            } else {
                theme.subtle
            };

            gfx.add_text(&line_number, visual_x, visual_y, color);
        }

        gfx.add_rect(
            self.gutter_bounds
                .unoffset_by(self.gutter_bounds)
                .right_border(gfx.border_width()),
            theme.border,
        );
    }

    fn update_indent_guide_x(
        &self,
        doc: &Doc,
        y: usize,
        indent_width: usize,
        indent_guide_x: &mut usize,
        ctx: &mut Ctx,
    ) {
        if !doc.is_line_whitespace(y) {
            let line = doc.get_line(y).unwrap_or_default();
            let line_start = doc.line_start(y);

            *indent_guide_x = ctx.gfx.measure_text(&line[..line_start]);

            return;
        }

        let previous_line_end = doc.line_end(y - 1);
        let is_at_block_start = doc.match_delimiter(previous_line_end, DelimiterKind::Start, ctx);

        if is_at_block_start {
            *indent_guide_x += indent_width;
        }
    }

    fn draw_indent_guides(
        &self,
        doc: &Doc,
        camera_position: VisualPosition,
        visible_lines: VisibleLines,
        ctx: &mut Ctx,
    ) {
        let Some(language) = ctx.config.get_language_for_doc(doc) else {
            return;
        };

        let indent_width = language.indent_width.measure(ctx.gfx);

        let theme = &ctx.config.theme;

        let mut indent_guide_x = 0;
        let mut indent_guide_start_y = visible_lines.min_y;

        while indent_guide_start_y > 0 && doc.is_line_whitespace(indent_guide_start_y) {
            indent_guide_start_y -= 1;
        }

        for y in indent_guide_start_y..visible_lines.min_y {
            self.update_indent_guide_x(doc, y, indent_width, &mut indent_guide_x, ctx);
        }

        for (i, y) in visible_lines.enumerate() {
            self.update_indent_guide_x(doc, y, indent_width, &mut indent_guide_x, ctx);

            let gfx = &mut ctx.gfx;

            for x in (indent_width..indent_guide_x).step_by(indent_width) {
                let visual_x = gfx.line_padding_x() + self.margin + gfx.glyph_width() * x as f32
                    - camera_position.x;

                let background_visual_y =
                    self.line_background_visual_y(i, visible_lines.offset, gfx);

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
        &self,
        (foreground, background): (Option<Color>, Option<Color>),
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

            let mut visual_x = gfx.line_padding_x() + self.margin - camera_position.x;
            let foreground_visual_y = self.line_foreground_visual_y(i, visible_lines.offset, gfx);
            let background_visual_y = self.line_background_visual_y(i, visible_lines.offset, gfx);

            if let Some(foreground) = foreground {
                gfx.add_text(line, visual_x, foreground_visual_y, foreground);
                continue;
            }

            let Some(highlights) = highlighted_lines
                .get(y)
                .map(HighlightedLine::highlights)
                .filter(|highlights| !highlights.is_empty())
            else {
                gfx.add_text(line, visual_x, foreground_visual_y, theme.normal);
                continue;
            };

            for highlight in highlights {
                let foreground = ctx
                    .config
                    .theme
                    .highlight_kind_to_color(highlight.foreground);
                let highlighted_text = &line[highlight.start..highlight.end];

                if let Some(highlight_background) = highlight.background {
                    let highlight_background = ctx
                        .config
                        .theme
                        .highlight_kind_to_color(highlight_background);

                    if Some(highlight_background) != background {
                        gfx.add_background(
                            highlighted_text,
                            visual_x,
                            background_visual_y,
                            highlight_background,
                        );
                    }
                }

                visual_x +=
                    gfx.add_text(highlighted_text, visual_x, foreground_visual_y, foreground);
            }
        }
    }

    fn draw_diagnostics(
        &self,
        doc: &Doc,
        camera_position: VisualPosition,
        visible_lines: VisibleLines,
        ctx: &mut Ctx,
    ) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        for language_server in ctx.lsp.iter_servers_mut() {
            // Reversed so that more severe diagnostics are drawn on top.
            for diagnostic in language_server.diagnostics_mut(doc).iter().rev() {
                let color = diagnostic.color(theme);
                let DecodedRange { start, end } = diagnostic.visible_range(doc);

                if start == end && start.y >= visible_lines.min_y && start.y < visible_lines.max_y {
                    let highlight_position =
                        self.position_to_visual(start, camera_position, doc, gfx);

                    gfx.add_zig_zag_underline(
                        highlight_position.x - gfx.glyph_width() / 2.0,
                        highlight_position.y + gfx.line_height(),
                        gfx.glyph_width(),
                        color,
                    );

                    continue;
                }

                let start = start.max(Position::new(0, visible_lines.min_y));
                let end = end.min(doc.line_end(visible_lines.max_y - 1));
                let mut position = start;

                while position < end {
                    if !diagnostic.contains_position(position, doc) {
                        position = doc.move_position(position, 1, 0, gfx);

                        continue;
                    }

                    let highlight_position =
                        self.position_to_visual(position, camera_position, doc, gfx);

                    let grapheme = doc.grapheme(position);
                    let grapheme_width = gfx.measure_text(grapheme);

                    gfx.add_zig_zag_underline(
                        highlight_position.x,
                        highlight_position.y + gfx.line_height(),
                        grapheme_width as f32 * gfx.glyph_width(),
                        color,
                    );

                    position = doc.move_position(position, 1, 0, gfx);
                }
            }
        }
    }

    fn draw_go_to_definition_hint(
        &self,
        doc: &Doc,
        camera_position: VisualPosition,
        ctx: &mut Ctx,
    ) -> Option<()> {
        doc.get_language_server_mut(ctx)?;

        let ui = &ctx.ui;
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        if !ctx.window.mods().contains(Mod::Ctrl) && !ctx.window.mods().contains(Mod::Cmd) {
            return None;
        }

        let visual_position = ctx.window.mouse_position();
        let position = self.visual_to_position_unclamped(visual_position, doc, ui, gfx)?;

        if GraphemeCategory::new(doc.grapheme(position)) != GraphemeCategory::Identifier {
            return None;
        }

        let selection = doc.select_current_word_at_position(position, gfx);

        let start = self.position_to_visual(selection.start, camera_position, doc, gfx);
        let end = self.position_to_visual(selection.end, camera_position, doc, gfx);

        gfx.add_underline(
            start.x,
            start.y + gfx.line_height(),
            end.x - start.x,
            theme.normal,
        );

        Some(())
    }

    fn draw_cursors(
        &self,
        doc: &Doc,
        camera_position: VisualPosition,
        visible_lines: VisibleLines,
        ctx: &mut Ctx,
    ) {
        for index in doc.cursor_indices() {
            let Some(selection) = doc.cursor(index).get_selection() else {
                continue;
            };

            self.draw_selection(selection, doc, camera_position, visible_lines, ctx);
        }

        let is_focused = ctx.ui.is_focused(self.widget_id);

        if !self.do_show_cursors(is_focused, ctx) {
            return;
        }

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        for (index, animation_state) in doc.cursor_indices().zip(&self.cursor_animation_states) {
            let trail_progress =
                self.cursor_animation_progress(ctx.time, animation_state.last_time);
            let trail_progress = ease_out_quart(trail_progress);

            let cursor_position =
                self.position_to_visual(doc.cursor(index).position, VisualPosition::ZERO, doc, gfx);
            let last_cursor_position = animation_state.last_position;

            let trail_position = last_cursor_position.lerp_to(cursor_position, trail_progress);

            let mut cursor_rect =
                Self::cursor_position_to_rect(cursor_position - camera_position, gfx);
            let mut trail_rect =
                Self::cursor_position_to_rect(trail_position - camera_position, gfx);

            if cursor_position.x != last_cursor_position.x
                && cursor_position.y != last_cursor_position.y
            {
                cursor_rect = cursor_rect.scale(trail_progress);
                trail_rect = trail_rect.scale(1.0 - trail_progress);
            }

            let cursor_quad: Quad = cursor_rect.into();
            let trail_quad = cursor_quad.expand_to_include(trail_rect.into());

            let mut trail_color = theme.normal;
            trail_color.a /= 2;

            gfx.add_quad(trail_quad, trail_color);
            gfx.add_quad(cursor_quad, theme.normal);
        }
    }

    fn draw_selection(
        &self,
        selection: Selection,
        doc: &Doc,
        camera_position: VisualPosition,
        visible_lines: VisibleLines,
        ctx: &mut Ctx,
    ) {
        let start = selection.start.max(Position::new(0, visible_lines.min_y));
        let end = selection.end.min(Position::new(0, visible_lines.max_y));

        if start >= end {
            return;
        }

        if start.y == end.y {
            self.draw_selection_line(
                Some(start.x),
                Some(end.x),
                start.y,
                doc,
                camera_position,
                ctx,
            );
        } else {
            self.draw_selection_line(Some(start.x), None, start.y, doc, camera_position, ctx);

            for y in start.y + 1..end.y {
                self.draw_selection_line(None, None, y, doc, camera_position, ctx);
            }

            self.draw_selection_line(None, Some(end.x), end.y, doc, camera_position, ctx);
        }
    }

    fn draw_selection_line(
        &self,
        start_x: Option<usize>,
        end_x: Option<usize>,
        y: usize,
        doc: &Doc,
        camera_position: VisualPosition,
        ctx: &mut Ctx,
    ) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;
        let lines = doc.lines();

        let start_x = start_x.unwrap_or_default();

        // Include the width of the newline character if we want
        // to highlight the entire rest of the line (end_x is None).
        let (end_x, newline_width) = if let Some(end_x) = end_x {
            (end_x, 0)
        } else {
            (lines[y].len(), 1)
        };

        if start_x == end_x + newline_width {
            return;
        }

        let highlight_position =
            self.position_to_visual(Position::new(start_x, y), camera_position, doc, gfx);

        let line_width = gfx.measure_text(&lines[y][start_x..end_x]) + newline_width;

        // Make the selection flush with the side of the doc.
        let padding_x = if start_x == 0 {
            gfx.line_padding_x()
        } else {
            0.0
        };

        let rect = Rect::new(
            highlight_position.x - padding_x,
            highlight_position.y,
            line_width as f32 * gfx.glyph_width() + padding_x,
            gfx.line_height(),
        );

        gfx.add_rect(rect, theme.selection);
    }

    fn cursor_position_to_rect(position: VisualPosition, gfx: &Gfx) -> Rect {
        let cursor_width = Self::cursor_width(gfx);

        Rect::new(
            position.x - cursor_width / 2.0,
            position.y,
            cursor_width,
            gfx.line_height(),
        )
    }

    fn do_show_cursors(&self, is_focused: bool, ctx: &Ctx) -> bool {
        is_focused && ctx.window.is_focused()
    }

    fn cursor_animation_progress(&self, time: f64, last_time: f64) -> f32 {
        ((time - last_time) * CURSOR_ANIMATION_SPEED) as f32
    }

    fn draw_scroll_bar(&self, doc: &Doc, camera_position: VisualPosition, ctx: &mut Ctx) {
        let bounds = ctx.ui.bounds(self.widget_id);

        if !doc.flags().contains(DocFlag::AllowScrollingPastBottom)
            && doc.lines().len() as f32 * ctx.gfx.line_height() + self.margin * 2.0 <= bounds.height
        {
            return;
        }

        let ui = &ctx.ui;
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        for language_server in ctx.lsp.iter_servers_mut() {
            // Reversed so that more severe diagnostics are drawn on top.
            for diagnostic in language_server.diagnostics_mut(doc).iter().rev() {
                if !diagnostic.is_problem() {
                    continue;
                }

                let color = diagnostic.color(theme);
                let DecodedRange { start, end } = diagnostic.range;

                gfx.add_rect(
                    self.doc_range_to_scrollbar_rect(start.y as f32, end.y as f32, doc, ui, gfx),
                    color,
                );
            }
        }

        for index in doc.cursor_indices() {
            let cursor_y = doc.cursor(index).position.y as f32;

            gfx.add_rect(
                self.doc_range_to_scrollbar_rect(cursor_y, cursor_y, doc, ui, gfx),
                theme.normal,
            );
        }

        let camera_line_y = camera_position.y / gfx.line_height();
        let doc_height_lines = self.doc_height_lines(ui, gfx);

        gfx.add_rect(
            self.doc_range_to_scrollbar_rect(
                camera_line_y,
                camera_line_y + doc_height_lines as f32,
                doc,
                ui,
                gfx,
            ),
            theme.emphasized,
        );
    }

    fn doc_range_to_scrollbar_rect(
        &self,
        start_y: f32,
        end_y: f32,
        doc: &Doc,
        ui: &Ui,
        gfx: &Gfx,
    ) -> Rect {
        let doc_height_lines = self.doc_height_lines(ui, gfx);
        let doc_len = doc.lines().len().max(doc_height_lines) as f32
            + (self.margin * 2.0 / gfx.line_height());

        let bounds = ui.bounds(self.widget_id);
        let width = gfx.glyph_width() / 2.0;
        let x = bounds.width - width;

        let start_y = start_y / doc_len * bounds.height;
        let end_y = end_y / doc_len * bounds.height;

        let start_y = start_y.floor();
        let end_y = end_y.floor();

        Rect::new(
            x,
            start_y,
            width,
            (end_y - start_y).max(gfx.border_width() * 2.0),
        )
    }
}
