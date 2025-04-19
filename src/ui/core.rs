use crate::{
    geometry::{rect::Rect, visual_position::VisualPosition},
    input::{
        input_handlers::{ActionHandler, GraphemeHandler, MouseScrollHandler, MousebindHandler},
        mouse_button::MouseButton,
        mousebind::Mousebind,
    },
    platform::window::Window,
    text::grapheme::GraphemeCursor,
};

pub struct Ui {
    next_widget_id: usize,
    hovered_widget_id: usize,
    focused_widget_ids: Vec<usize>,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            next_widget_id: 0,
            hovered_widget_id: 0,
            focused_widget_ids: Vec::new(),
        }
    }

    pub fn update(&mut self, focusable_widgets: &mut [&mut Widget], window: &mut Window) {
        let mut focused_widget_index = None;
        let mut hovered_widget_index = None;

        let mut mousebind_handler = window.get_mousebind_handler();

        while let Some(mousebind) = mousebind_handler.next(window) {
            let position = VisualPosition::new(mousebind.x, mousebind.y);
            let widget_index = Self::get_widget_index_at(position, focusable_widgets);
            hovered_widget_index = widget_index;

            if let Mousebind {
                button: Some(MouseButton::Left),
                is_drag: false,
                ..
            } = mousebind
            {
                focused_widget_index = widget_index;
            }

            mousebind_handler.unprocessed(window, mousebind);
        }

        let mut mouse_scroll_handler = window.get_mouse_scroll_handler();

        while let Some(mouse_scroll) = mouse_scroll_handler.next(window) {
            let position = VisualPosition::new(mouse_scroll.x, mouse_scroll.y);
            let widget_index = Self::get_widget_index_at(position, focusable_widgets);
            hovered_widget_index = widget_index;

            mouse_scroll_handler.unprocessed(window, mouse_scroll);
        }

        if let Some(focused_widget_index) = focused_widget_index {
            self.focus(focusable_widgets[focused_widget_index]);
        }

        if let Some(hovered_widget_index) = hovered_widget_index {
            self.hover(focusable_widgets[hovered_widget_index]);
        }
    }

    fn get_widget_index_at(position: VisualPosition, widgets: &[&mut Widget]) -> Option<usize> {
        let mut widget_index = None;

        for (index, widget) in widgets.iter().enumerate() {
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

        widget_index
    }

    fn focused_widget_id(&self) -> usize {
        self.focused_widget_ids.last().copied().unwrap_or(0)
    }

    pub fn focus(&mut self, widget: &mut Widget) {
        self.remove_from_focused(widget);
        self.show(widget);
        self.focused_widget_ids.push(widget.id);
    }

    pub fn unfocus(&mut self, widget: &Widget) {
        if self.focused_widget_id() != widget.id {
            return;
        }

        self.focused_widget_ids.pop();
    }

    fn remove_from_focused(&mut self, widget: &Widget) {
        let index = self
            .focused_widget_ids
            .iter()
            .position(|widget_id| *widget_id == widget.id);

        if let Some(index) = index {
            self.focused_widget_ids.remove(index);
        }
    }

    pub fn hover(&mut self, widget: &Widget) {
        self.hovered_widget_id = widget.id;
    }

    pub fn show(&mut self, widget: &mut Widget) {
        widget.is_visible = true;
    }

    pub fn hide(&mut self, widget: &mut Widget) {
        self.remove_from_focused(widget);
        widget.is_visible = false;
    }

    pub fn is_focused(&self, widget: &Widget) -> bool {
        self.focused_widget_id() == widget.id && widget.is_visible
    }

    pub fn is_hovered(&self, widget: &Widget) -> bool {
        self.hovered_widget_id == widget.id && widget.is_visible
    }

    pub fn get_grapheme_handler(&self, widget: &Widget, window: &Window) -> GraphemeHandler {
        if self.is_focused(widget) {
            window.get_grapheme_handler()
        } else {
            GraphemeHandler::new(GraphemeCursor::new(0, 0))
        }
    }

    pub fn get_action_handler(&self, widget: &Widget, window: &Window) -> ActionHandler {
        if self.is_focused(widget) {
            window.get_action_handler()
        } else {
            ActionHandler::new(0)
        }
    }

    pub fn get_mousebind_handler(&self, widget: &Widget, window: &Window) -> MousebindHandler {
        if self.is_focused(widget) {
            window.get_mousebind_handler()
        } else {
            MousebindHandler::new(0)
        }
    }

    pub fn get_mouse_scroll_handler(&self, widget: &Widget, window: &Window) -> MouseScrollHandler {
        if self.is_hovered(widget) {
            window.get_mouse_scroll_handler()
        } else {
            MouseScrollHandler::new(0)
        }
    }
}

pub struct Widget {
    is_visible: bool,
    bounds: Vec<Rect>,
    id: usize,
}

impl Widget {
    pub fn new(ui: &mut Ui, is_visible: bool) -> Self {
        let widget = Self {
            bounds: vec![Rect::ZERO],
            id: ui.next_widget_id,
            is_visible,
        };

        ui.next_widget_id += 1;

        widget
    }

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
