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
pub struct WidgetId {
    index: usize,
    generation: usize,
}

impl WidgetId {
    pub const ROOT: Self = Self {
        index: 0,
        generation: 0,
    };
}

#[derive(Debug)]
pub struct WidgetSettings {
    pub is_visible: bool,
    pub is_component: bool,
}

impl Default for WidgetSettings {
    fn default() -> Self {
        Self {
            is_visible: true,
            is_component: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct Widget {
    pub bounds: Rect,

    settings: WidgetSettings,
    parent_id: Option<WidgetId>,
    child_ids: Vec<WidgetId>,
}

#[derive(Debug, Default)]
struct WidgetSlot {
    widget: Widget,
    generation: usize,
}

pub struct Ui {
    widget_slots: Vec<WidgetSlot>,
    hovered_widget_id: WidgetId,
    focus_history: Vec<WidgetId>,
    unused_widget_indices: Vec<usize>,
}

impl Ui {
    pub fn new() -> Self {
        let root = Widget {
            bounds: Rect::ZERO,

            settings: Default::default(),
            parent_id: None,
            child_ids: vec![],
        };

        Self {
            widget_slots: vec![WidgetSlot {
                widget: root,
                generation: 0,
            }],
            hovered_widget_id: WidgetId::ROOT,
            focus_history: Vec::new(),
            unused_widget_indices: Vec::new(),
        }
    }

    pub fn new_widget(&mut self, parent_id: WidgetId, options: WidgetSettings) -> WidgetId {
        let (widget_id, widget) = if let Some(index) = self.unused_widget_indices.pop() {
            let slot = &mut self.widget_slots[index];

            let widget_id = WidgetId {
                index,
                generation: slot.generation,
            };

            (widget_id, &mut slot.widget)
        } else {
            let index = self.widget_slots.len();

            self.widget_slots.push(WidgetSlot::default());

            let widget = &mut self.widget_slots[index].widget;

            let widget_id = WidgetId {
                index,
                generation: 0,
            };

            (widget_id, widget)
        };

        widget.bounds = Rect::ZERO;

        widget.settings = options;
        widget.parent_id = Some(parent_id);
        widget.child_ids.clear();

        self.widget_mut(parent_id).child_ids.push(widget_id);

        widget_id
    }

    pub fn remove_widget(&mut self, widget_id: WidgetId) {
        if widget_id == WidgetId::ROOT {
            return;
        }

        if self.widget_slots[widget_id.index].generation != widget_id.generation {
            return;
        }

        if self.focused_widget_id() == widget_id {
            self.unfocus(widget_id);
        }

        if let Some(parent_id) = self.widget(widget_id).parent_id {
            let parent = self.widget_mut(parent_id);

            if let Some(index) = parent
                .child_ids
                .iter()
                .position(|child_id| *child_id == widget_id)
            {
                parent.child_ids.remove(index);
            }
        }

        for i in 0..self.widget(widget_id).child_ids.len() {
            let child_id = self.widget(widget_id).child_ids[i];

            self.remove_widget(child_id);
        }

        self.widget_slots[widget_id.index].generation += 1;
        self.unused_widget_indices.push(widget_id.index);
    }

    pub fn update(&mut self, window: &mut Window) {
        let mut focused_widget_id = None;
        let mut hovered_widget_id = None;

        let mut mousebind_handler = window.mousebind_handler();

        while let Some(mousebind) = mousebind_handler.next(window) {
            let position = VisualPosition::new(mousebind.x, mousebind.y);
            let widget_id = self.get_widget_id_at(position, WidgetId::ROOT);
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
            let widget_id = self.get_widget_id_at(position, WidgetId::ROOT);
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

    fn get_widget_id_at(&self, position: VisualPosition, widget_id: WidgetId) -> Option<WidgetId> {
        if !self.is_visible(widget_id) {
            return None;
        }

        let widget = self.widget(widget_id);

        for child_id in widget.child_ids.iter().rev() {
            if let Some(widget_id) = self.get_widget_id_at(position, *child_id) {
                return Some(widget_id);
            }
        }

        if widget.bounds.contains_position(position) {
            Some(widget_id)
        } else {
            None
        }
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

    fn hover(&mut self, widget_id: WidgetId) {
        self.hovered_widget_id = widget_id;
    }

    pub fn show(&mut self, widget_id: WidgetId) {
        self.widget_mut(widget_id).settings.is_visible = true;
    }

    pub fn hide(&mut self, widget_id: WidgetId) {
        self.remove_from_focused(widget_id);
        self.widget_mut(widget_id).settings.is_visible = false;
    }

    pub fn is_focused(&self, widget_id: WidgetId) -> bool {
        self.focused_widget_id() == widget_id
    }

    pub fn is_in_focused_hierarchy(&self, widget_id: WidgetId) -> bool {
        let widget = self.widget(widget_id);

        if widget.settings.is_component {
            if let Some(parent_id) = widget.parent_id {
                return self.is_in_focused_hierarchy(parent_id);
            }
        }

        let mut focused_hierarchy_id = self.focused_widget_id();

        loop {
            if focused_hierarchy_id == widget_id {
                return true;
            }

            if let Some(parent_id) = self.widget(focused_hierarchy_id).parent_id {
                focused_hierarchy_id = parent_id;
            } else {
                return false;
            }
        }
    }

    pub fn widget(&self, widget_id: WidgetId) -> &Widget {
        let slot = &self.widget_slots[widget_id.index];
        assert!(slot.generation == widget_id.generation);

        &slot.widget
    }

    pub fn widget_mut(&mut self, widget_id: WidgetId) -> &mut Widget {
        let slot = &mut self.widget_slots[widget_id.index];
        assert!(slot.generation == widget_id.generation);

        &mut slot.widget
    }

    pub fn is_hovered(&self, widget_id: WidgetId) -> bool {
        self.hovered_widget_id == widget_id
    }

    pub fn is_visible(&self, widget_id: WidgetId) -> bool {
        let widget = self.widget(widget_id);

        if !widget.settings.is_visible {
            return false;
        }

        if let Some(parent_id) = widget.parent_id {
            self.is_visible(parent_id)
        } else {
            true
        }
    }

    pub fn grapheme_handler(&self, widget_id: WidgetId, window: &Window) -> GraphemeHandler {
        if self.is_in_focused_hierarchy(widget_id) {
            window.grapheme_handler()
        } else {
            GraphemeHandler::new(GraphemeCursor::new(0, 0))
        }
    }

    pub fn action_handler(&self, widget_id: WidgetId, window: &Window) -> ActionHandler {
        if self.is_in_focused_hierarchy(widget_id) {
            window.action_handler()
        } else {
            ActionHandler::new(0)
        }
    }

    pub fn mousebind_handler(&self, widget_id: WidgetId, window: &Window) -> MousebindHandler {
        if self.is_hovered(widget_id) {
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
