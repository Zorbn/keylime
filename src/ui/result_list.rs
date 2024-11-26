use std::vec::Drain;

use crate::{
    config::Config,
    geometry::{rect::Rect, side::SIDE_ALL, visual_position::VisualPosition},
    input::{key::Key, keybind::Keybind, mouse_button::MouseButton, mousebind::Mousebind},
    platform::{gfx::Gfx, window::Window},
};

use super::camera::{Camera, RECENTER_DISTANCE};

pub enum ResultListInput {
    None,
    Complete,
    Submit { mods: u8 },
    Close,
}

pub struct ResultList<T> {
    pub results: Vec<T>,
    selected_result_index: usize,
    handled_selected_result_index: usize,

    max_visible_results: usize,
    result_bounds: Rect,
    results_bounds: Rect,

    camera: Camera,
}

impl<T> ResultList<T> {
    pub fn new(max_visible_results: usize) -> Self {
        Self {
            results: Vec::new(),
            selected_result_index: 0,
            handled_selected_result_index: 0,

            max_visible_results,
            result_bounds: Rect::zero(),
            results_bounds: Rect::zero(),

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
        window: &mut Window,
        is_visible: bool,
        is_focused: bool,
        dt: f32,
    ) -> ResultListInput {
        let mut input = ResultListInput::None;

        self.selected_result_index = self
            .selected_result_index
            .clamp(0, self.results.len().saturating_sub(1));

        if is_visible {
            self.handle_mouse_inputs(&mut input, window);
        }

        if is_focused {
            self.handle_keybinds(&mut input, window);
        }

        self.update_camera(dt);

        input
    }

    fn handle_mouse_inputs(&mut self, input: &mut ResultListInput, window: &mut Window) {
        let mut mouse_handler = window.get_mousebind_handler();

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

            self.selected_result_index = clicked_result_index;
            self.mark_selected_result_handled();

            if mousebind.button.is_some() {
                *input = ResultListInput::Submit { mods };
            }
        }

        let mut mouse_scroll_handler = window.get_mouse_scroll_handler();

        while let Some(mouse_scroll) = mouse_scroll_handler.next(window) {
            let position = VisualPosition::new(mouse_scroll.x, mouse_scroll.y);

            if mouse_scroll.is_horizontal || !self.results_bounds.contains_position(position) {
                mouse_scroll_handler.unprocessed(window, mouse_scroll);
                continue;
            }

            let delta = mouse_scroll.delta * self.result_bounds.height;
            self.camera.vertical.scroll(delta);
        }
    }

    fn handle_keybinds(&mut self, input: &mut ResultListInput, window: &mut Window) {
        let mut keybind_handler = window.get_keybind_handler();

        while let Some(keybind) = keybind_handler.next(window) {
            match keybind {
                Keybind {
                    key: Key::Escape,
                    mods: 0,
                } => *input = ResultListInput::Close,
                Keybind {
                    key: Key::Enter,
                    mods,
                } => *input = ResultListInput::Submit { mods },
                Keybind {
                    key: Key::Tab,
                    mods: 0,
                } => *input = ResultListInput::Complete,
                Keybind {
                    key: Key::Up,
                    mods: 0,
                } => {
                    if self.selected_result_index > 0 {
                        self.selected_result_index -= 1;
                    }
                }
                Keybind {
                    key: Key::Down,
                    mods: 0,
                } => {
                    if self.selected_result_index < self.results.len() - 1 {
                        self.selected_result_index += 1;
                    }
                }
                _ => keybind_handler.unprocessed(window, keybind),
            }
        }
    }

    fn update_camera(&mut self, dt: f32) {
        let target_y =
            (self.selected_result_index as f32 + 0.5) * self.result_bounds.height - self.camera.y();
        let max_y = (self.results.len() as f32 * self.result_bounds.height
            - self.results_bounds.height)
            .max(0.0);

        let scroll_border_top = self.result_bounds.height * RECENTER_DISTANCE as f32;
        let scroll_border_bottom = self.results_bounds.height - scroll_border_top;

        let can_recenter = self.selected_result_index != self.handled_selected_result_index;
        self.mark_selected_result_handled();

        self.camera.vertical.update(
            target_y,
            max_y,
            self.results_bounds.height,
            scroll_border_top,
            scroll_border_bottom,
            can_recenter,
            dt,
        );
    }

    pub fn draw<'a, C: Iterator<Item = char>>(
        &'a mut self,
        config: &Config,
        gfx: &mut Gfx,
        result_to_chars: fn(&'a T) -> C,
    ) {
        gfx.begin(Some(self.results_bounds));

        gfx.add_bordered_rect(
            self.results_bounds.unoffset_by(self.results_bounds),
            SIDE_ALL,
            &config.theme.background,
            &config.theme.border,
        );

        let camera_y = self.camera.y().floor();

        let min_y = self.min_visible_result_index();
        let sub_line_offset_y = camera_y - min_y as f32 * self.result_bounds.height;
        let max_y = self.max_visible_result_index();

        for (i, y) in (min_y..max_y).enumerate() {
            let visual_y = i as f32 * self.result_bounds.height
                + (self.result_bounds.height - gfx.glyph_height()) / 2.0
                - sub_line_offset_y;

            let color = if y == self.selected_result_index {
                &config.theme.keyword
            } else {
                &config.theme.normal
            };

            let result = &self.results[y];
            let chars = result_to_chars(result);

            gfx.add_text(chars, gfx.glyph_width(), visual_y, color);
        }

        gfx.end();
    }

    pub fn drain(&mut self) -> Drain<T> {
        self.selected_result_index = 0;
        self.camera.reset();

        self.results.drain(..)
    }

    pub fn get_selected_result(&self) -> Option<&T> {
        self.results.get(self.selected_result_index)
    }

    pub fn mark_selected_result_handled(&mut self) {
        self.handled_selected_result_index = self.selected_result_index;
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
