use crate::{
    geometry::rect::Rect,
    input::input_handlers::{ActionHandler, GraphemeHandler, MouseScrollHandler, MousebindHandler},
    platform::window::Window,
    text::grapheme::GraphemeCursor,
};

use super::Ui;

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

    pub fn get_grapheme_handler(&self, ui: &Ui, window: &Window) -> GraphemeHandler {
        if self.is_focused(ui, window) {
            window.get_grapheme_handler()
        } else {
            GraphemeHandler::new(GraphemeCursor::new(0, 0))
        }
    }

    pub fn get_action_handler(&self, ui: &Ui, window: &Window) -> ActionHandler {
        if self.is_focused(ui, window) {
            window.get_action_handler()
        } else {
            ActionHandler::new(0)
        }
    }

    pub fn get_mousebind_handler(&self, ui: &Ui, window: &Window) -> MousebindHandler {
        if self.is_focused(ui, window) {
            window.get_mousebind_handler()
        } else {
            MousebindHandler::new(0)
        }
    }

    pub fn get_mouse_scroll_handler(&self, ui: &Ui, window: &Window) -> MouseScrollHandler {
        if self.is_hovered(ui) {
            window.get_mouse_scroll_handler()
        } else {
            MouseScrollHandler::new(0)
        }
    }

    pub fn take_hover(&mut self, ui: &mut Ui) {
        ui.hovered_widget_id = self.id;
    }

    pub fn take_focus(&mut self, ui: &mut Ui) {
        if ui.focused_widget_id != self.id {
            ui.last_focused_widget_id = ui.focused_widget_id;
        }

        ui.focused_widget_id = self.id;
    }

    pub fn release_focus(&mut self, ui: &mut Ui) {
        if ui.focused_widget_id != self.id {
            return;
        }

        ui.focused_widget_id = ui.last_focused_widget_id;
        ui.last_focused_widget_id = self.id;
    }

    pub fn is_focused(&self, ui: &Ui, window: &Window) -> bool {
        window.is_focused() && self.id == ui.focused_widget_id && self.is_visible
    }

    pub fn is_hovered(&self, ui: &Ui) -> bool {
        self.id == ui.hovered_widget_id && self.is_visible
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
