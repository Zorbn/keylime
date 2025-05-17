use std::ops::Deref;

use crate::{
    geometry::{rect::Rect, visual_position::VisualPosition},
    input::{
        input_handlers::{ActionHandler, GraphemeHandler, MouseScrollHandler, MousebindHandler},
        mouse_button::MouseButton,
        mousebind::{MouseClickKind, Mousebind},
    },
    platform::{gfx::Gfx, window::Window},
    pool::{Pooled, VEC_WIDGET_ID_POOL},
    text::grapheme::GraphemeCursor,
};

use super::color::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetId {
    Name(&'static str),
    NameWithIndex(&'static str, usize),
}

impl WidgetId {
    pub fn name(&self) -> &'static str {
        match self {
            WidgetId::Name(name) => name,
            WidgetId::NameWithIndex(name, _) => name,
        }
    }
}

#[derive(Debug)]
pub enum ContainerDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Default)]
pub struct WidgetLayout {
    // If position is set, the widget is layed out relative to
    // the container and won't affect the bounds of future widgets.
    pub position: Option<VisualPosition>,
    // Used for horizontal/vertical layouts respectively, if a size
    // is not set then the widget will occupy all of the remaining space.
    pub width: Option<f32>,
    pub height: Option<f32>,
}

struct Container {
    direction: ContainerDirection,
    is_reversed: bool,
}

impl Container {
    pub fn new(direction: ContainerDirection) -> Self {
        Self {
            direction,
            is_reversed: false,
        }
    }
}

const MAX_FOCUS_HISTORY_LEN: usize = 8;

pub struct Ui {
    focus_history: Vec<Pooled<Vec<WidgetId>>>,
    id_stack: Vec<WidgetId>,
    is_in_widget: bool,
    left_click_position: Option<VisualPosition>,
    container_stack: Vec<Container>,
    bounds_stack: Vec<Rect>,
    layout_stack: Vec<WidgetLayout>,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            focus_history: Vec::new(),
            id_stack: Vec::new(),
            is_in_widget: false,
            left_click_position: None,
            container_stack: Vec::new(),
            bounds_stack: Vec::new(),
            layout_stack: Vec::new(),
        }
    }

    pub fn begin(&mut self, clear_color: Color, window: &mut Window, gfx: &mut Gfx) {
        let mut mousebind_handler = window.mousebind_handler();

        while let Some(mousebind) = mousebind_handler.next(window) {
            if let Mousebind {
                button: Some(MouseButton::Left),
                x,
                y,
                kind: MouseClickKind::Press,
                ..
            } = mousebind
            {
                self.begin_left_click_focus(VisualPosition::new(x, y));
            }

            mousebind_handler.unprocessed(window, mousebind);
        }

        let bounds = Rect::new(0.0, 0.0, gfx.width(), gfx.height());

        self.bounds_stack.push(bounds);
        self.container_stack
            .push(Container::new(ContainerDirection::Vertical));
        gfx.begin_frame(clear_color);
    }

    pub fn end(&mut self, gfx: &mut Gfx) {
        gfx.end_frame();
        self.container_stack.pop();
        self.bounds_stack.pop();

        self.end_left_click_focus();
    }

    // TODO: Consider accepting &'static str names instead of widget ids in begin_container/widget and counting index automatically within each container.
    pub fn begin_container(
        &mut self,
        id: WidgetId,
        layout: WidgetLayout,
        direction: ContainerDirection,
    ) {
        assert!(!self.is_in_widget);

        self.begin_bounds(layout);
        self.id_stack.push(id);
        self.container_stack.push(Container::new(direction));
    }

    pub fn end_container(&mut self) {
        assert!(!self.is_in_widget);

        self.id_stack.pop();
        self.container_stack.pop();
        self.end_bounds();
    }

    pub fn reverse_container(&mut self) {
        if let Some(container) = self.container_stack.last_mut() {
            container.is_reversed = !container.is_reversed;
        }
    }

    pub fn begin_widget(&mut self, id: WidgetId, layout: WidgetLayout, gfx: &mut Gfx) {
        assert!(!self.is_in_widget);
        self.is_in_widget = true;

        self.begin_bounds(layout);
        self.id_stack.push(id);

        let bounds = self.bounds();

        if self
            .left_click_position
            .is_some_and(|position| bounds.contains_position(position))
        {
            self.end_left_click_focus();
        }

        gfx.begin(Some(bounds));
    }

    pub fn end_widget(&mut self, gfx: &mut Gfx) {
        assert!(self.is_in_widget);
        self.is_in_widget = false;

        gfx.end();
        self.id_stack.pop();
        self.end_bounds();
    }

    fn begin_bounds(&mut self, layout: WidgetLayout) {
        let container_bounds = self.bounds_stack.last().unwrap();
        let container = self.container_stack.last().unwrap();

        let width = match container.direction {
            ContainerDirection::Horizontal => layout.width.unwrap_or(container_bounds.width),
            ContainerDirection::Vertical => container_bounds.width,
        };

        let height = match container.direction {
            ContainerDirection::Horizontal => container_bounds.height,
            ContainerDirection::Vertical => layout.height.unwrap_or(container_bounds.height),
        };

        let (x, y) = if let Some(position) = layout.position {
            (position.x, position.y)
        } else if container.is_reversed {
            match container.direction {
                ContainerDirection::Horizontal => {
                    (container_bounds.right() - width, container_bounds.y)
                }
                ContainerDirection::Vertical => {
                    (container_bounds.x, container_bounds.bottom() - height)
                }
            }
        } else {
            (container_bounds.x, container_bounds.y)
        };

        self.layout_stack.push(layout);
        self.bounds_stack.push(Rect::new(x, y, width, height));
    }

    fn end_bounds(&mut self) {
        let layout = self.layout_stack.pop().unwrap();
        let bounds = self.bounds_stack.pop().unwrap();

        if layout.position.is_some() {
            return;
        }

        let container_bounds = self.bounds_stack.last_mut().unwrap();
        let container = self.container_stack.last().unwrap();

        *container_bounds = match container.direction {
            ContainerDirection::Horizontal => {
                if container.is_reversed {
                    container_bounds.shrink_right_by(bounds)
                } else {
                    container_bounds.shrink_left_by(bounds)
                }
            }
            ContainerDirection::Vertical => {
                if container.is_reversed {
                    container_bounds.shrink_bottom_by(bounds)
                } else {
                    container_bounds.shrink_top_by(bounds)
                }
            }
        }
    }

    pub fn bounds(&self) -> Rect {
        *self.bounds_stack.last().unwrap()
    }

    pub fn focus(&mut self, id_stack: &[WidgetId]) {
        self.end_left_click_focus();

        Self::focus_history_push(&mut self.focus_history, id_stack);
    }

    fn focus_history_push(focus_history: &mut Vec<Pooled<Vec<WidgetId>>>, id_stack: &[WidgetId]) {
        while focus_history.len() >= MAX_FOCUS_HISTORY_LEN {
            focus_history.remove(0);
        }

        let index = focus_history
            .iter()
            .position(|history_stack| history_stack.deref() == id_stack);

        if let Some(index) = index {
            focus_history.remove(index);
        }

        focus_history.push(
            VEC_WIDGET_ID_POOL.init_item(|focus_stack| focus_stack.extend_from_slice(id_stack)),
        );
    }

    // TODO: When a command palette is closed or a pane is removed it needs to be unfocused if it is focused.
    pub fn unfocus(&mut self, id_stack: &[WidgetId]) {
        self.end_left_click_focus();

        if !self.is_stack_focused(id_stack) {
            return;
        }

        self.focus_history.pop();
    }

    fn begin_left_click_focus(&mut self, position: VisualPosition) {
        if self.left_click_position.is_none() {
            // Add a placeholder so that the previous widget doesn't
            // continue to be focused until we reach the widget that was clicked.
            Self::focus_history_push(&mut self.focus_history, &[]);
        }

        self.left_click_position = Some(position);
    }

    fn end_left_click_focus(&mut self) {
        if self.left_click_position.is_none() {
            return;
        }

        self.left_click_position = None;

        // Remove the placeholder.
        self.focus_history.pop();
        Self::focus_history_push(&mut self.focus_history, &self.id_stack);
    }

    pub fn is_focused(&self) -> bool {
        self.is_stack_focused(&self.id_stack)
    }

    pub fn is_stack_focused(&self, id_stack: &[WidgetId]) -> bool {
        self.focused_stack().is_some_and(|focus_stack| {
            id_stack.len() <= focus_stack.len() && id_stack == &focus_stack[..id_stack.len()]
        })
    }

    pub fn focused_stack(&self) -> Option<&[WidgetId]> {
        self.focus_history
            .last()
            .map(|focus_stack| focus_stack.as_slice())
    }

    pub fn is_hovered(&self, window: &Window) -> bool {
        self.bounds().contains_position(window.mouse_position())
    }

    pub fn grapheme_handler(&self, window: &Window) -> GraphemeHandler {
        if self.is_focused() {
            window.grapheme_handler()
        } else {
            GraphemeHandler::new(GraphemeCursor::new(0, 0))
        }
    }

    pub fn action_handler(&self, window: &Window) -> ActionHandler {
        if self.is_focused() {
            window.action_handler()
        } else {
            ActionHandler::new(0)
        }
    }

    pub fn mousebind_handler(&self, window: &Window) -> MousebindHandler {
        if self.is_hovered(window) {
            window.mousebind_handler()
        } else {
            MousebindHandler::new(0)
        }
    }

    pub fn mouse_scroll_handler(&self, window: &Window) -> MouseScrollHandler {
        if self.is_hovered(window) {
            window.mouse_scroll_handler()
        } else {
            MouseScrollHandler::new(0)
        }
    }
}
