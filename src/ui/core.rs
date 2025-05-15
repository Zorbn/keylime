use crate::{
    geometry::{rect::Rect, visual_position::VisualPosition},
    input::{
        input_handlers::{ActionHandler, GraphemeHandler, MouseScrollHandler, MousebindHandler},
        mouse_button::MouseButton,
        mousebind::{MouseClickKind, Mousebind},
    },
    platform::window::Window,
    text::grapheme::GraphemeCursor,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WidgetId(usize);

impl WidgetId {
    pub const ROOT: Self = Self(0);
}

pub struct Ui {
    widgets: Vec<Widget>,
    hovered_widget_id: WidgetId,
    focus_history: Vec<WidgetId>,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            widgets: Vec::new(),
            hovered_widget_id: WidgetId::ROOT,
            focus_history: Vec::new(),
        }
    }

    pub fn new_widget(&mut self, is_visible: bool) -> WidgetId {
        let widget_id = WidgetId(self.widgets.len());

        self.widgets.push(Widget {
            bounds: vec![Rect::ZERO],
            is_visible,
        });

        widget_id
    }

    pub fn update(&mut self, window: &mut Window) {
        let mut focused_widget_id = None;
        let mut hovered_widget_id = None;

        let mut mousebind_handler = window.mousebind_handler();

        while let Some(mousebind) = mousebind_handler.next(window) {
            let position = VisualPosition::new(mousebind.x, mousebind.y);
            let widget_id = self.get_widget_id_at(position);
            hovered_widget_id = widget_id;

            if let Mousebind {
                button: Some(MouseButton::Left),
                kind: MouseClickKind::Press,
                ..
            } = mousebind
            {
                focused_widget_id = widget_id;
            }

            mousebind_handler.unprocessed(window, mousebind);
        }

        let mut mouse_scroll_handler = window.mouse_scroll_handler();

        while let Some(mouse_scroll) = mouse_scroll_handler.next(window) {
            let position = VisualPosition::new(mouse_scroll.x, mouse_scroll.y);
            let widget_id = self.get_widget_id_at(position);
            hovered_widget_id = widget_id;

            mouse_scroll_handler.unprocessed(window, mouse_scroll);
        }

        if let Some(focused_widget_id) = focused_widget_id {
            self.focus(focused_widget_id);
        }

        if let Some(hovered_widget_id) = hovered_widget_id {
            self.hover(hovered_widget_id);
        }
    }

    fn get_widget_id_at(&self, position: VisualPosition) -> Option<WidgetId> {
        let mut widget_index = None;

        for (index, widget) in self.widgets.iter().enumerate() {
            if !widget.is_visible {
                continue;
            }

            for bounds in widget.all_bounds() {
                if bounds.contains_position(position) {
                    widget_index = Some(index);

                    break;
                }
            }
        }

        widget_index.map(WidgetId)
    }

    fn focused_widget_id(&self) -> WidgetId {
        self.focus_history.last().copied().unwrap_or_default()
    }

    pub fn focus(&mut self, widget_id: WidgetId) {
        self.remove_from_focused(widget_id);
        self.show(widget_id);
        self.focus_history.push(widget_id);
    }

    pub fn unfocus(&mut self, widget_id: WidgetId) {
        if self.focused_widget_id() != widget_id {
            return;
        }

        self.focus_history.pop();
    }

    fn remove_from_focused(&mut self, widget_id: WidgetId) {
        let index = self
            .focus_history
            .iter()
            .position(|focused_id| *focused_id == widget_id);

        if let Some(index) = index {
            self.focus_history.remove(index);
        }
    }

    pub fn hover(&mut self, widget_id: WidgetId) {
        self.hovered_widget_id = widget_id;
    }

    pub fn show(&mut self, widget_id: WidgetId) {
        self.widget_mut(widget_id).is_visible = true;
    }

    pub fn hide(&mut self, widget_id: WidgetId) {
        self.remove_from_focused(widget_id);
        self.widget_mut(widget_id).is_visible = false;
    }

    pub fn is_focused(&self, widget_id: WidgetId) -> bool {
        self.focused_widget_id() == widget_id
    }

    pub fn widget(&self, widget_id: WidgetId) -> &Widget {
        &self.widgets[widget_id.0]
    }

    pub fn widget_mut(&mut self, widget_id: WidgetId) -> &mut Widget {
        &mut self.widgets[widget_id.0]
    }

    pub fn is_hovered(&self, widget_id: WidgetId) -> bool {
        self.hovered_widget_id == widget_id
    }

    pub fn grapheme_handler(&self, widget_id: WidgetId, window: &Window) -> GraphemeHandler {
        if self.is_focused(widget_id) {
            window.grapheme_handler()
        } else {
            GraphemeHandler::new(GraphemeCursor::new(0, 0))
        }
    }

    pub fn action_handler(&self, widget_id: WidgetId, window: &Window) -> ActionHandler {
        if self.is_focused(widget_id) {
            window.action_handler()
        } else {
            ActionHandler::new(0)
        }
    }

    pub fn mousebind_handler(&self, widget_id: WidgetId, window: &Window) -> MousebindHandler {
        if self.is_focused(widget_id) {
            window.mousebind_handler()
        } else {
            MousebindHandler::new(0)
        }
    }

    pub fn mouse_scroll_handler(&self, widget_id: WidgetId, window: &Window) -> MouseScrollHandler {
        if self.is_hovered(widget_id) {
            window.mouse_scroll_handler()
        } else {
            MouseScrollHandler::new(0)
        }
    }
}

pub struct Widget {
    is_visible: bool,
    bounds: Vec<Rect>,
}

impl Widget {
    pub fn layout(&mut self, bounds: &[Rect]) {
        self.bounds.clear();

        if bounds.is_empty() {
            self.bounds.push(Rect::ZERO);
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

    pub fn is_visible(&self) -> bool {
        self.is_visible
    }
}
