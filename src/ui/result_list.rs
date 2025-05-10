use std::vec::Drain;

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
    platform::{gfx::Gfx, window::Window},
};

use super::{
    camera::{Camera, RECENTER_DISTANCE},
    color::Color,
    core::{Ui, Widget},
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
    result_bounds: Rect,
    results_bounds: Rect,

    camera: Camera,
}

impl<T> ResultList<T> {
    pub fn new(max_visible_results: usize) -> Self {
        Self {
            results: FocusList::new(),
            handled_focused_index: None,

            max_visible_results,
            result_bounds: Rect::ZERO,
            results_bounds: Rect::ZERO,

            camera: Camera::new(),
        }
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx) {
        self.result_bounds = Rect::new(0.0, 0.0, bounds.width, gfx.line_height() * 1.25);

        self.results_bounds = Rect::new(
            bounds.x,
            bounds.y,
            bounds.width,
            self.result_bounds.height * self.results.len().min(self.max_visible_results) as f32,
        )
        .floor();
    }

    pub fn offset_by(&mut self, bounds: Rect) {
        self.results_bounds = self.results_bounds.offset_by(bounds);
    }

    pub fn update(
        &mut self,
        widget: &Widget,
        ui: &mut Ui,
        window: &mut Window,
        can_be_visible: bool,
        can_be_focused: bool,
    ) -> ResultListInput {
        let mut input = ResultListInput::None;

        if can_be_visible && widget.is_visible() {
            self.handle_mouse_inputs(&mut input, widget, ui, window);
        }

        if can_be_focused && ui.is_focused(widget) {
            self.handle_keybinds(&mut input, widget, ui, window);
        }

        input
    }

    fn handle_mouse_inputs(
        &mut self,
        input: &mut ResultListInput,
        widget: &Widget,
        ui: &mut Ui,
        window: &mut Window,
    ) {
        let mut mouse_handler = ui.get_mousebind_handler(widget, window);

        while let Some(mousebind) = mouse_handler.next(window) {
            let position = VisualPosition::new(mousebind.x, mousebind.y);
            let results_bounds = self.results_bounds;

            let Mousebind {
                button: None | Some(MouseButton::Left),
                mods,
                ..
            } = mousebind
            else {
                mouse_handler.unprocessed(window, mousebind);
                continue;
            };

            if !results_bounds.contains_position(position) {
                mouse_handler.unprocessed(window, mousebind);
                continue;
            }

            let clicked_result_index = ((position.y + self.camera.y() - results_bounds.y)
                / self.result_bounds.height) as usize;

            if clicked_result_index >= self.results.len() {
                continue;
            }

            self.results.set_focused_index(clicked_result_index);
            self.mark_focused_handled();

            if mousebind.button.is_some() {
                let kind = if mods.contains(Mod::Shift) {
                    ResultListSubmitKind::Alternate
                } else {
                    ResultListSubmitKind::Normal
                };

                *input = ResultListInput::Submit { kind };
            }
        }

        let mut mouse_scroll_handler = ui.get_mouse_scroll_handler(widget, window);

        while let Some(mouse_scroll) = mouse_scroll_handler.next(window) {
            let position = VisualPosition::new(mouse_scroll.x, mouse_scroll.y);

            if mouse_scroll.is_horizontal || !self.results_bounds.contains_position(position) {
                mouse_scroll_handler.unprocessed(window, mouse_scroll);
                continue;
            }

            let delta = mouse_scroll.delta * self.result_bounds.height;
            self.camera.vertical.scroll(delta, mouse_scroll.is_precise);
        }
    }

    fn handle_keybinds(
        &mut self,
        input: &mut ResultListInput,
        widget: &Widget,
        ui: &mut Ui,
        window: &mut Window,
    ) {
        let mut action_handler = ui.get_action_handler(widget, window);

        while let Some(action) = action_handler.next(window) {
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
                action_keybind!(key: Up, mods: Mods::NONE) => self.results.focus_previous(),
                action_keybind!(key: Down, mods: Mods::NONE) => self.results.focus_next(),
                _ => action_handler.unprocessed(window, action),
            }
        }
    }

    pub fn update_camera(&mut self, dt: f32) {
        let focused_index = self.results.focused_index();

        let target_y = (focused_index as f32 + 0.5) * self.result_bounds.height - self.camera.y();
        let max_y = (self.results.len() as f32 * self.result_bounds.height
            - self.results_bounds.height)
            .max(0.0);

        let scroll_border_top = self.result_bounds.height * RECENTER_DISTANCE as f32;
        let scroll_border_bottom = self.results_bounds.height - scroll_border_top;

        let can_recenter = Some(focused_index) != self.handled_focused_index;
        self.mark_focused_handled();

        self.camera.vertical.update(
            target_y,
            max_y,
            self.results_bounds.height,
            scroll_border_top..=scroll_border_bottom,
            can_recenter,
            dt,
        );
    }

    pub fn draw<'a>(
        &'a mut self,
        ctx: &mut Ctx,
        mut display_result: impl FnMut(&'a T, &Theme) -> (&'a str, Color),
    ) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        gfx.begin(Some(self.results_bounds));

        gfx.add_bordered_rect(
            self.results_bounds.unoffset_by(self.results_bounds),
            Sides::ALL,
            theme.background,
            theme.border,
        );

        let camera_y = self.camera.y().floor();

        let min_y = self.min_visible_result_index();
        let sub_line_offset_y = camera_y - min_y as f32 * self.result_bounds.height;
        let max_y = self.max_visible_result_index();

        for (i, y) in (min_y..max_y).enumerate() {
            let background_visual_y = i as f32 * self.result_bounds.height - sub_line_offset_y;

            let foreground_visual_y =
                background_visual_y + (self.result_bounds.height - gfx.glyph_height()) / 2.0;

            let Some(result) = self.results.get(y) else {
                continue;
            };

            let (text, color) = display_result(result, theme);

            if y == self.results.focused_index() {
                gfx.add_rect(
                    Rect::new(
                        0.0,
                        background_visual_y,
                        self.result_bounds.width,
                        self.result_bounds.height,
                    )
                    .add_margin(-gfx.border_width()),
                    theme.border,
                );
            }

            gfx.add_text(text, gfx.glyph_width(), foreground_visual_y, color);
        }

        gfx.end();
    }

    pub fn reset_focused(&mut self) {
        self.results.set_focused_index(0);
        self.handled_focused_index = None;
    }

    pub fn drain(&mut self) -> Drain<T> {
        self.reset_focused();
        self.camera.reset();

        self.results.drain()
    }

    pub fn mark_focused_handled(&mut self) {
        self.handled_focused_index = Some(self.results.focused_index());
    }

    pub fn bounds(&self) -> Rect {
        self.results_bounds
    }

    pub fn is_animating(&self) -> bool {
        self.camera.is_moving()
    }

    pub fn min_visible_result_index(&self) -> usize {
        (self.camera.y().floor() / self.result_bounds.height) as usize
    }

    pub fn max_visible_result_index(&self) -> usize {
        let max_y = ((self.camera.y().floor() + self.results_bounds.height)
            / self.result_bounds.height) as usize
            + 1;

        max_y.min(self.results.len())
    }
}
