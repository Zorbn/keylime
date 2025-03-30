use unicode_segmentation::GraphemeCursor;

use crate::{
    geometry::rect::Rect,
    input::input_handlers::{ActionHandler, GraphemeHandler, MouseScrollHandler, MousebindHandler},
    platform::{gfx::Gfx, window::Window},
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

    pub fn get_grapheme_handler(&self, ui: &UiHandle) -> GraphemeHandler {
        if self.is_focused(ui) {
            ui.window.get_grapheme_handler()
        } else {
            GraphemeHandler::new(GraphemeCursor::new(0, 0, true))
        }
    }

    pub fn get_action_handler(&self, ui: &UiHandle) -> ActionHandler {
        if self.is_focused(ui) {
            ui.window.get_action_handler()
        } else {
            ActionHandler::new(0)
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
        if self.is_hovered(ui) {
            ui.window.get_mouse_scroll_handler()
        } else {
            MouseScrollHandler::new(0)
        }
    }

    pub fn take_hover(&mut self, ui: &mut UiHandle) {
        ui.inner.hovered_widget_id = self.id;
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

    pub fn is_hovered(&self, ui: &UiHandle) -> bool {
        self.id == ui.inner.hovered_widget_id && self.is_visible
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

pub struct WidgetHandle<'a, 'b> {
    inner: &'a mut Widget,
    ui: &'a mut UiHandle<'b>,
}

impl<'a, 'b> WidgetHandle<'a, 'b> {
    pub fn new(widget: &'a mut Widget, ui: &'a mut UiHandle<'b>) -> Self {
        Self { inner: widget, ui }
    }

    pub fn get_grapheme_handler(&self) -> GraphemeHandler {
        self.inner.get_grapheme_handler(self.ui)
    }

    pub fn get_action_handler(&self) -> ActionHandler {
        self.inner.get_action_handler(self.ui)
    }

    pub fn get_mousebind_handler(&self) -> MousebindHandler {
        self.inner.get_mousebind_handler(self.ui)
    }

    pub fn get_mouse_scroll_handler(&self) -> MouseScrollHandler {
        self.inner.get_mouse_scroll_handler(self.ui)
    }

    pub fn window(&mut self) -> &mut Window {
        self.ui.window
    }

    pub fn gfx(&mut self) -> &mut Gfx {
        self.ui.gfx()
    }
}
