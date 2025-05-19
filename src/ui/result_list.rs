use std::{
    ops::{Deref, DerefMut},
    vec::Drain,
};

use crate::{
    config::theme::Theme,
    ctx::Ctx,
    geometry::{rect::Rect, sides::Sides, visual_position::VisualPosition},
    input::{
        action::action_keybind,
        mods::{Mod, Mods},
        mouse_button::MouseButton,
        mousebind::Mousebind,
    },
    platform::gfx::Gfx,
};

use super::{
    camera::{Camera, RECENTER_DISTANCE},
    color::Color,
    core::{Ui, WidgetId, WidgetSettings},
    focus_list::FocusList,
};

#[derive(Debug, PartialEq, Eq)]
pub enum ResultListSubmitKind {
    Normal,
    Alternate,
}

pub enum ResultListInput {
    None,
    Complete,
    Submit { kind: ResultListSubmitKind },
    Close,
}

pub struct ResultList<T> {
    pub results: FocusList<T>,
    handled_focused_index: Option<usize>,

    max_visible_results: usize,
    do_show_when_empty: bool,

    result_bounds: Rect,
    widget_id: WidgetId,

    camera: Camera,
}

impl<T> ResultList<T> {
    pub fn new(
        max_visible_results: usize,
        do_show_when_empty: bool,
        parent_id: WidgetId,
        ui: &mut Ui,
    ) -> Self {
        Self {
            results: FocusList::new(),
            handled_focused_index: None,

            max_visible_results,
            do_show_when_empty,

            result_bounds: Rect::ZERO,
            widget_id: ui.new_widget(
                parent_id,
                WidgetSettings {
                    is_component: true,
                    ..Default::default()
                },
            ),

            camera: Camera::new(),
        }
    }

    pub fn layout(&mut self, bounds: Rect, ui: &mut Ui, gfx: &Gfx) {
        ui.set_shown(
            self.widget_id,
            self.do_show_when_empty || !self.results.is_empty(),
        );

        self.result_bounds = Rect::new(0.0, 0.0, bounds.width, gfx.line_height() * 1.25);

        ui.widget_mut(self.widget_id).bounds = Rect::new(
            bounds.x,
            bounds.y,
            bounds.width,
            self.result_bounds.height * self.len().min(self.max_visible_results) as f32,
        )
        .floor();
    }

    pub fn offset_by(&self, bounds: Rect, ui: &mut Ui) {
        let widget = ui.widget_mut(self.widget_id);
        widget.bounds = widget.bounds.offset_by(bounds);
    }

    pub fn update(&mut self, ctx: &mut Ctx) -> ResultListInput {
        let mut input = ResultListInput::None;

        self.handle_mouse_inputs(&mut input, ctx);
        self.handle_keybinds(&mut input, ctx);

        input
    }

    fn handle_mouse_inputs(&mut self, input: &mut ResultListInput, ctx: &mut Ctx) {
        let bounds = ctx.ui.widget(self.widget_id).bounds;

        let mut mouse_handler = ctx.ui.mousebind_handler(self.widget_id, ctx.window);

        while let Some(mousebind) = mouse_handler.next(ctx.window) {
            let position = VisualPosition::new(mousebind.x, mousebind.y);

            let Mousebind {
                button: None | Some(MouseButton::Left),
                mods,
                ..
            } = mousebind
            else {
                mouse_handler.unprocessed(ctx.window, mousebind);
                continue;
            };

            if !self.try_focus_position(position, ctx) {
                mouse_handler.unprocessed(ctx.window, mousebind);
                continue;
            }

            if mousebind.button.is_some() {
                let kind = if mods.contains(Mod::Shift) {
                    ResultListSubmitKind::Alternate
                } else {
                    ResultListSubmitKind::Normal
                };

                *input = ResultListInput::Submit { kind };
            }
        }

        let mut mouse_scroll_handler = ctx.ui.mouse_scroll_handler(self.widget_id, ctx.window);

        while let Some(mouse_scroll) = mouse_scroll_handler.next(ctx.window) {
            let position = VisualPosition::new(mouse_scroll.x, mouse_scroll.y);

            if mouse_scroll.is_horizontal || !bounds.contains_position(position) {
                mouse_scroll_handler.unprocessed(ctx.window, mouse_scroll);
                continue;
            }

            let delta = mouse_scroll.delta * self.result_bounds.height;
            self.camera.vertical.scroll(delta, mouse_scroll.is_precise);
            self.try_focus_position(position, ctx);
        }
    }

    fn try_focus_position(&mut self, position: VisualPosition, ctx: &mut Ctx) -> bool {
        let bounds = ctx.ui.widget(self.widget_id).bounds;

        if !bounds.contains_position(position) {
            return false;
        }

        let clicked_result_index =
            ((position.y + self.camera.y() - bounds.y) / self.result_bounds.height) as usize;

        if clicked_result_index >= self.len() {
            return false;
        }

        self.set_focused_index(clicked_result_index);
        self.mark_focused_handled();

        true
    }

    fn handle_keybinds(&mut self, input: &mut ResultListInput, ctx: &mut Ctx) {
        let mut keybind_handler = ctx.ui.keybind_handler(self.widget_id, ctx.window);

        while let Some(action) = keybind_handler.next_action(ctx) {
            match action {
                action_keybind!(key: Escape, mods: Mods::NONE) => *input = ResultListInput::Close,
                action_keybind!(key: Enter, mods: Mods::NONE) => {
                    *input = ResultListInput::Submit {
                        kind: ResultListSubmitKind::Normal,
                    }
                }
                action_keybind!(key: Enter, mods: Mods::SHIFT) => {
                    *input = ResultListInput::Submit {
                        kind: ResultListSubmitKind::Alternate,
                    }
                }
                action_keybind!(key: Tab, mods: Mods::NONE) => *input = ResultListInput::Complete,
                action_keybind!(key: Up, mods: Mods::NONE) => self.focus_previous(),
                action_keybind!(key: Down, mods: Mods::NONE) => self.focus_next(),
                _ => keybind_handler.unprocessed(ctx.window, action.keybind),
            }
        }
    }

    pub fn update_camera(&mut self, ui: &Ui, dt: f32) {
        let focused_index = self.focused_index();
        let bounds = ui.widget(self.widget_id).bounds;

        let target_y = (focused_index as f32 + 0.5) * self.result_bounds.height - self.camera.y();
        let max_y = (self.len() as f32 * self.result_bounds.height - bounds.height).max(0.0);

        let scroll_border_top = self.result_bounds.height * RECENTER_DISTANCE as f32;
        let scroll_border_bottom = bounds.height - scroll_border_top;

        let can_recenter = Some(focused_index) != self.handled_focused_index;
        self.mark_focused_handled();

        self.camera.vertical.update(
            target_y,
            max_y,
            bounds.height,
            scroll_border_top..=scroll_border_bottom,
            can_recenter,
            dt,
        );
    }

    pub fn draw<'a>(
        &'a self,
        ctx: &mut Ctx,
        mut display_result: impl FnMut(&'a T, &Theme) -> (&'a str, Color),
    ) {
        if !ctx.ui.is_visible(self.widget_id) {
            return;
        }

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        let bounds = ctx.ui.widget(self.widget_id).bounds;

        gfx.begin(Some(bounds));

        gfx.add_bordered_rect(
            bounds.unoffset_by(bounds),
            Sides::ALL,
            theme.background,
            theme.border,
        );

        let camera_y = self.camera.y().floor();

        let min_y = self.min_visible_result_index();
        let sub_line_offset_y = camera_y - min_y as f32 * self.result_bounds.height;
        let max_y = self.max_visible_result_index(ctx.ui);

        for (i, y) in (min_y..max_y).enumerate() {
            let background_visual_y = i as f32 * self.result_bounds.height - sub_line_offset_y;

            let foreground_visual_y =
                background_visual_y + (self.result_bounds.height - gfx.glyph_height()) / 2.0;

            let Some(result) = self.get(y) else {
                continue;
            };

            let (text, color) = display_result(result, theme);

            if y == self.focused_index() {
                gfx.add_rect(
                    Rect::new(
                        0.0,
                        background_visual_y,
                        self.result_bounds.width,
                        self.result_bounds.height,
                    )
                    .add_margin(-gfx.border_width()),
                    theme.emphasized,
                );
            }

            gfx.add_text(text, gfx.glyph_width(), foreground_visual_y, color);
        }

        gfx.end();
    }

    pub fn reset_focused(&mut self) {
        self.set_focused_index(0);
        self.handled_focused_index = None;
    }

    pub fn drain(&mut self) -> Drain<T> {
        self.reset_focused();
        self.camera.reset();

        self.results.drain()
    }

    pub fn mark_focused_handled(&mut self) {
        self.handled_focused_index = Some(self.focused_index());
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    pub fn is_animating(&self) -> bool {
        self.camera.is_moving()
    }

    pub fn min_visible_result_index(&self) -> usize {
        (self.camera.y().floor() / self.result_bounds.height) as usize
    }

    pub fn max_visible_result_index(&self, ui: &Ui) -> usize {
        let bounds = ui.widget(self.widget_id).bounds;
        let max_y =
            ((self.camera.y().floor() + bounds.height) / self.result_bounds.height) as usize + 1;

        max_y.min(self.len())
    }
}

impl<T> Deref for ResultList<T> {
    type Target = FocusList<T>;

    fn deref(&self) -> &Self::Target {
        &self.results
    }
}

impl<T> DerefMut for ResultList<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.results
    }
}
