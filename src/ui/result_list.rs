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
        mouse_scroll::MouseScroll,
        mousebind::{Mousebind, MousebindKind},
    },
    platform::gfx::Gfx,
    ui::{
        camera::{CameraAxis, CameraRecenterRequest},
        msg::Msg,
    },
};

use super::{
    camera::RECENTER_DISTANCE,
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

    camera: CameraAxis,
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
            widget_id: ui.new_widget(parent_id, Default::default()),

            camera: CameraAxis::new(),
        }
    }

    // pub fn layout(&mut self, bounds: Rect, ui: &mut Ui, gfx: &Gfx) {
    //     ui.set_shown(
    //         self.widget_id,
    //         self.do_show_when_empty || !self.results.is_empty(),
    //     );

    //     self.result_bounds = Rect::new(0.0, 0.0, bounds.width, gfx.line_height() * 1.25);

    //     ui.widget_mut(self.widget_id).bounds = Rect::new(
    //         bounds.x,
    //         bounds.y,
    //         bounds.width,
    //         self.result_bounds.height * self.len().min(self.max_visible_results) as f32,
    //     )
    //     .floor();
    // }

    // pub fn offset_by(&self, bounds: Rect, ui: &mut Ui) {
    //     let widget = ui.widget_mut(self.widget_id);
    //     widget.bounds = widget.bounds.offset_by(bounds);
    // }

    pub fn receive_msgs(&mut self, ctx: &mut Ctx) -> ResultListInput {
        let mut input = ResultListInput::None;

        while let Some(msg) = ctx.ui.msg(self.widget_id) {
            match msg {
                Msg::Mousebind(Mousebind {
                    button: button @ (None | Some(MouseButton::Left)),
                    x,
                    y,
                    mods,
                    kind: MousebindKind::Press | MousebindKind::Move,
                    ..
                }) => {
                    let position = VisualPosition::new(x, y);

                    if !self.try_focus_position(position, ctx) {
                        ctx.ui.skip(self.widget_id, msg);
                        continue;
                    }

                    if button.is_some() {
                        let kind = if mods.contains(Mod::Shift) {
                            ResultListSubmitKind::Alternate
                        } else {
                            ResultListSubmitKind::Normal
                        };

                        input = ResultListInput::Submit { kind };
                    }
                }
                Msg::MouseScroll(MouseScroll {
                    delta,
                    is_horizontal,
                    kind,
                    x,
                    y,
                }) => {
                    let bounds = ctx.ui.bounds(self.widget_id);
                    let position = VisualPosition::new(x, y);

                    if is_horizontal || !bounds.contains_position(position) {
                        ctx.ui.skip(self.widget_id, msg);
                        continue;
                    }

                    let delta = delta * self.result_bounds.height;
                    self.camera.scroll(delta, kind);
                }
                Msg::Action(action_keybind!(key: Escape, mods: Mods::NONE)) => {
                    input = ResultListInput::Close
                }
                Msg::Action(action_keybind!(key: Enter, mods: Mods::NONE)) => {
                    input = ResultListInput::Submit {
                        kind: ResultListSubmitKind::Normal,
                    }
                }
                Msg::Action(action_keybind!(key: Enter, mods: Mods::SHIFT)) => {
                    input = ResultListInput::Submit {
                        kind: ResultListSubmitKind::Alternate,
                    }
                }
                Msg::Action(action_keybind!(key: Tab, mods: Mods::NONE)) => {
                    input = ResultListInput::Complete
                }
                Msg::Action(action_keybind!(key: Up, mods: Mods::NONE)) => self.focus_previous(),
                Msg::Action(action_keybind!(key: Down, mods: Mods::NONE)) => self.focus_next(),
                _ => ctx.ui.skip(self.widget_id, msg),
            }
        }

        input
    }

    fn try_focus_position(&mut self, position: VisualPosition, ctx: &Ctx) -> bool {
        let bounds = ctx.ui.bounds(self.widget_id);

        if !bounds.contains_position(position) {
            return false;
        }

        let clicked_result_index =
            ((position.y + self.camera.position() - bounds.y) / self.result_bounds.height) as usize;

        if clicked_result_index >= self.len() {
            return false;
        }

        self.set_focused_index(clicked_result_index);
        self.mark_focused_handled();

        true
    }

    pub fn animate(&mut self, ctx: &Ctx, dt: f32) {
        let focused_index = self.focused_index();
        let bounds = ctx.ui.bounds(self.widget_id);

        let target_y =
            (focused_index as f32 + 0.5) * self.result_bounds.height - self.camera.position();
        let max_y = (self.len() as f32 * self.result_bounds.height - bounds.height).max(0.0);

        let recenter_request = CameraRecenterRequest {
            can_start: Some(focused_index) != self.handled_focused_index,
            target_position: target_y,
            scroll_border: self.result_bounds.height * RECENTER_DISTANCE as f32,
        };

        self.mark_focused_handled();

        self.camera
            .animate(recenter_request, max_y, bounds.height, dt);
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

        let bounds = ctx.ui.bounds(self.widget_id);

        gfx.begin(Some(bounds));

        gfx.add_bordered_rect(
            bounds.unoffset_by(bounds),
            Sides::ALL,
            theme.background,
            theme.border,
        );

        let camera_y = self.camera.position().floor();

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

    pub fn drain(&mut self) -> Drain<'_, T> {
        self.set_focused_index(0);
        self.results.drain()
    }

    pub fn reset(&mut self) {
        self.drain();
        self.camera.reset();
    }

    fn mark_focused_handled(&mut self) {
        self.handled_focused_index = Some(self.focused_index());
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    pub fn is_animating(&self) -> bool {
        self.camera.is_moving()
    }

    pub fn min_visible_result_index(&self) -> usize {
        (self.camera.position().floor() / self.result_bounds.height) as usize
    }

    pub fn max_visible_result_index(&self, ui: &Ui) -> usize {
        let bounds = ui.bounds(self.widget_id);
        let max_y = ((self.camera.position().floor() + bounds.height) / self.result_bounds.height)
            as usize
            + 1;

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
