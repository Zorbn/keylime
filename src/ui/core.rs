use std::f32::consts::E;

use crate::{
    geometry::{rect::Rect, visual_position::VisualPosition},
    input::{
        input_handlers::{ActionHandler, GraphemeHandler, MouseScrollHandler, MousebindHandler},
        mouse_button::MouseButton,
        mousebind::{MouseClickKind, Mousebind},
    },
    platform::{gfx::Gfx, window::Window},
    text::grapheme::GraphemeCursor,
};

use super::color::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetId {
    Name(&'static str),
    NameWithIndex(&'static str, usize),
    Component,
}

// TODO: Also allow choosing whether to add to the beginning or end of containers,
// eg. the app's main container should stack up from the bottom (status bar, then terminal, then editor fills in the rest).
// eg. if we add a menu bar at the top of the app, we would add the status bar and terminal to the bottom, add the menu bar to the top, then add the editor last and have it fill remaining space.
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
    has_focused_widget: bool,
}

impl Container {
    pub fn new(direction: ContainerDirection) -> Self {
        Self {
            direction,
            has_focused_widget: false,
        }
    }
}

pub struct Ui {
    focus_history: Vec<WidgetId>,
    id_stack: Vec<WidgetId>,
    is_in_widget: bool,
    left_click_position: Option<VisualPosition>,
    container_stack: Vec<Container>,
    bounds_stack: Vec<Rect>,
    current_layout: WidgetLayout,
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
            current_layout: WidgetLayout::default(),
        }
    }

    pub fn begin(&mut self, clear_color: Color, window: &mut Window, gfx: &mut Gfx) {
        self.left_click_position = None;

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
                self.left_click_position = Some(VisualPosition::new(x, y));
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
    }

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
        let container = self.container_stack.pop().unwrap();
        self.end_bounds();

        if container.has_focused_widget {
            self.container_stack.last_mut().unwrap().has_focused_widget = true;
        }
    }

    pub fn begin_widget(&mut self, id: WidgetId, layout: WidgetLayout, gfx: &mut Gfx) {
        assert!(!self.is_in_widget);
        self.is_in_widget = true;

        self.begin_bounds(layout);

        let bounds = self.bounds();

        if let Some(position) = self.left_click_position {
            println!(
                "position: {:?}, bounds: {:?}, id: {:?}",
                position, bounds, id
            );
        }

        if self
            .left_click_position
            .is_some_and(|position| bounds.contains_position(position))
        {
            self.left_click_position = None;

            let id = self
                .id_stack
                .iter()
                .rev()
                .copied()
                .find(|id| *id != WidgetId::Component);

            if let Some(id) = id {
                self.focus(id);
            }
        }

        self.id_stack.push(id);

        gfx.begin(Some(bounds));
    }

    pub fn end_widget(&mut self, gfx: &mut Gfx) {
        assert!(self.is_in_widget);
        self.is_in_widget = false;

        gfx.end();

        if self.is_focused() {
            self.container_stack.last_mut().unwrap().has_focused_widget = true;
        }

        self.id_stack.pop();

        self.end_bounds();
    }

    fn begin_bounds(&mut self, layout: WidgetLayout) {
        // TODO: Support choosing if you want to be at the beginning or end of the parent container.
        let container_bounds = self.bounds_stack.last().unwrap();
        let container = self.container_stack.last().unwrap();

        let (x, y) = if let Some(position) = layout.position {
            (position.x, position.y)
        } else {
            (container_bounds.x, container_bounds.y)
        };

        let width = match container.direction {
            ContainerDirection::Horizontal => layout.width.unwrap_or(container_bounds.width),
            ContainerDirection::Vertical => container_bounds.width,
        };

        let height = match container.direction {
            ContainerDirection::Horizontal => container_bounds.height,
            ContainerDirection::Vertical => layout.height.unwrap_or(container_bounds.height),
        };

        self.current_layout = layout;
        self.bounds_stack.push(Rect::new(x, y, width, height));
    }

    fn end_bounds(&mut self) {
        let bounds = self.bounds_stack.pop().unwrap();

        if self.current_layout.position.is_some() {
            return;
        }

        let container_bounds = self.bounds_stack.last_mut().unwrap();
        let container = self.container_stack.last().unwrap();

        *container_bounds = match container.direction {
            ContainerDirection::Horizontal => container_bounds.shrink_left_by(bounds),
            ContainerDirection::Vertical => container_bounds.shrink_top_by(bounds),
        }
    }

    pub fn bounds(&self) -> Rect {
        *self.bounds_stack.last().unwrap()
    }

    pub fn focus(&mut self, id: WidgetId) {
        println!("trying to focus: {:?}", id);

        if id == WidgetId::Component {
            return;
        }

        let index = self
            .focus_history
            .iter()
            .position(|history_id| *history_id == id);

        if let Some(index) = index {
            self.focus_history.remove(index);
        }

        self.focus_history.push(id);
    }

    // TODO: When a command palette is closed or a pane is removed it needs to be unfocused if it is focused.
    pub fn unfocus(&mut self, id: WidgetId) {
        if id == WidgetId::Component {
            return;
        }

        if self.focus_history.last() != Some(&id) {
            return;
        }

        self.focus_history.pop();
    }

    pub fn is_focused(&self) -> bool {
        let id = self
            .id_stack
            .iter()
            .rev()
            .copied()
            .find(|id| *id != WidgetId::Component);

        self.focus_history.last() == id.as_ref()
            || self
                .container_stack
                .last()
                .as_ref()
                .is_some_and(|container| container.has_focused_widget)
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
