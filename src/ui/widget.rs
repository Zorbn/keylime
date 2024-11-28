use crate::{
    geometry::rect::Rect,
    input::input_handlers::{CharHandler, KeybindHandler, MouseScrollHandler, MousebindHandler},
};

use super::{Ui, UiHandle};

pub struct Widget {
    pub is_visible: bool,

    bounds: Vec<Rect>,
    id: usize,
}

impl Widget {
    pub fn new(ui: &mut Ui, is_visible: bool) -> Self {
        let widget = Self {
            bounds: vec![Rect::zero()],
            id: ui.next_widget_id,
            is_visible,
        };

        ui.next_widget_id += 1;

        widget
    }

    pub fn get_char_handler(&self, ui: &UiHandle) -> CharHandler {
        if self.is_focused(ui) {
            ui.window.get_char_handler()
        } else {
            CharHandler::new(0)
        }
    }

    pub fn get_keybind_handler(&self, ui: &UiHandle) -> KeybindHandler {
        if self.is_focused(ui) {
            ui.window.get_keybind_handler()
        } else {
            KeybindHandler::new(0)
        }
    }

    pub fn get_mousebind_handler(&self, ui: &UiHandle) -> MousebindHandler {
        if self.is_focused(ui) {
            ui.window.get_mousebind_handler()
        } else {
            MousebindHandler::new(0)
        }
    }

    pub fn get_mouse_scroll_handler(&self, ui: &UiHandle) -> MouseScrollHandler {
        if self.is_focused(ui) {
            ui.window.get_mouse_scroll_handler()
        } else {
            MouseScrollHandler::new(0)
        }
    }

    pub fn take_focus(&mut self, ui: &mut UiHandle) {
        if ui.inner.focused_widget_id != self.id {
            ui.inner.last_focused_widget_id = ui.inner.focused_widget_id;
        }

        ui.inner.focused_widget_id = self.id;
    }

    pub fn release_focus(&mut self, ui: &mut UiHandle) {
        if ui.inner.focused_widget_id != self.id {
            return;
        }

        ui.inner.focused_widget_id = ui.inner.last_focused_widget_id;
        ui.inner.last_focused_widget_id = self.id;
    }

    pub fn is_focused(&self, ui: &UiHandle) -> bool {
        ui.window.is_focused() && self.id == ui.inner.focused_widget_id && self.is_visible
    }

    pub fn layout(&mut self, bounds: &[Rect]) {
        self.bounds.clear();

        if bounds.is_empty() {
            self.bounds.push(Rect::zero());
        } else {
            self.bounds.extend_from_slice(bounds);
        }
    }

    pub fn bounds(&self) -> Rect {
        self.bounds[0]
    }

    pub fn all_bounds(&self) -> &[Rect] {
        &self.bounds
    }
}
