use std::{cmp::Ordering, vec::Drain};

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
    core::{Ui, WidgetId},
};

#[derive(Debug, PartialEq, Eq)]
pub enum ResultListSubmitKind {
    Normal,
    Alternate,
}

pub enum ResultListInput {
    None,
    FocusChanged,
    Complete,
    Submit { kind: ResultListSubmitKind },
    Close,
}

pub struct ResultList<T> {
    widget_id: WidgetId,

    items: Vec<T>,
    focused_index: usize,
    handled_focused_index: Option<usize>,

    camera: CameraAxis,
}

impl<T> ResultList<T> {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        Self {
            widget_id: ui.new_widget(parent_id, Default::default()),

            items: Vec::new(),
            focused_index: 0,
            handled_focused_index: None,

            camera: CameraAxis::new(),
        }
    }

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

                    let delta = delta * Self::result_height(ctx.gfx);
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

        if matches!(input, ResultListInput::None)
            && Some(self.focused_index()) != self.handled_focused_index
        {
            input = ResultListInput::FocusChanged;
        }

        input
    }

    fn try_focus_position(&mut self, position: VisualPosition, ctx: &Ctx) -> bool {
        let bounds = ctx.ui.bounds(self.widget_id);

        if !bounds.contains_position(position) {
            return false;
        }

        let clicked_result_index = ((position.y + self.camera.position() - bounds.y)
            / Self::result_height(ctx.gfx)) as usize;

        if clicked_result_index >= self.len() {
            return false;
        }

        self.set_focused_index(clicked_result_index);
        self.mark_focused_handled();

        true
    }

    pub fn update(&mut self, ctx: &Ctx, dt: f32) {
        let focused_index = self.focused_index();
        let bounds = ctx.ui.bounds(self.widget_id);
        let result_height = Self::result_height(ctx.gfx);

        let target_y = (focused_index as f32 + 0.5) * result_height - self.camera.position();
        let max_y = (self.len() as f32 * result_height - bounds.height).max(0.0);

        let recenter_request = CameraRecenterRequest {
            can_start: Some(focused_index) != self.handled_focused_index,
            target_position: target_y,
            scroll_border: result_height * RECENTER_DISTANCE as f32,
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

        let ui = &ctx.ui;
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        let bounds = ctx.ui.bounds(self.widget_id);
        let result_height = Self::result_height(gfx);

        gfx.begin(Some(bounds));

        gfx.add_bordered_rect(
            bounds.relative_to(bounds),
            Sides::ALL,
            theme.background,
            theme.border,
        );

        let camera_y = self.camera.position().floor();

        let min_y = self.min_visible_result_index(gfx);
        let sub_line_offset_y = camera_y - min_y as f32 * result_height;
        let max_y = self.max_visible_result_index(ui, gfx);

        for (i, y) in (min_y..max_y).enumerate() {
            let background_visual_y = i as f32 * result_height - sub_line_offset_y;

            let foreground_visual_y =
                background_visual_y + (result_height - gfx.glyph_height()) / 2.0;

            let Some(result) = self.get(y) else {
                continue;
            };

            let (text, color) = display_result(result, theme);

            if y == self.focused_index() {
                gfx.add_rect(
                    Rect::new(0.0, background_visual_y, bounds.width, result_height)
                        .add_margin(-gfx.border_width()),
                    theme.emphasized,
                );
            }

            gfx.add_text(text, gfx.glyph_width(), foreground_visual_y, color);
        }

        gfx.end();
    }

    pub fn focus_next(&mut self) {
        if self.focused_index < self.items.len().saturating_sub(1) {
            self.focused_index += 1;
        } else {
            self.focused_index = 0;
        }
    }

    pub fn focus_previous(&mut self) {
        if self.focused_index > 0 {
            self.focused_index -= 1;
        } else {
            self.focused_index = self.items.len().saturating_sub(1);
        }
    }

    fn clamp_focused(&mut self) {
        self.focused_index = self.focused_index.min(self.items.len().saturating_sub(1));
    }

    pub fn insert(&mut self, index: usize, item: T) {
        if index < self.items.len() && self.focused_index >= index {
            self.focused_index += 1;
        }

        self.items.insert(index, item);
    }

    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    pub fn append(&mut self, other: &mut Vec<T>) {
        self.items.append(other);
    }

    pub fn remove(&mut self) -> Option<T> {
        let item =
            (self.focused_index < self.items.len()).then(|| self.items.remove(self.focused_index));

        self.clamp_focused();

        item
    }

    pub fn sort_by(&mut self, compare: impl FnMut(&T, &T) -> Ordering) {
        self.items.sort_by(compare);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn get_focused(&self) -> Option<&T> {
        self.items.get(self.focused_index)
    }

    pub fn get_focused_mut(&mut self) -> Option<&mut T> {
        self.items.get_mut(self.focused_index)
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.items.get_mut(index)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn set_focused_index(&mut self, index: usize) {
        self.focused_index = index;
        self.clamp_focused();
    }

    pub fn focused_index(&self) -> usize {
        self.focused_index
    }

    pub fn drain(&mut self) -> Drain<'_, T> {
        self.focused_index = 0;
        self.items.drain(..)
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

    pub fn min_visible_result_index(&self, gfx: &Gfx) -> usize {
        (self.camera.position().floor() / Self::result_height(gfx)) as usize
    }

    pub fn max_visible_result_index(&self, ui: &Ui, gfx: &Gfx) -> usize {
        let bounds = ui.bounds(self.widget_id);
        let result_height = Self::result_height(gfx);
        let max_y = ((self.camera.position().floor() + bounds.height) / result_height) as usize + 1;

        max_y.min(self.len())
    }

    pub fn desired_height(&self, max_visible_results: usize, gfx: &Gfx) -> f32 {
        Self::result_height(gfx) * self.items.len().min(max_visible_results) as f32
    }

    pub fn result_height(gfx: &Gfx) -> f32 {
        gfx.line_height() * 1.25
    }
}
