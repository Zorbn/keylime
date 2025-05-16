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

pub struct Ui {
    sibling_focus_history: Vec<usize>,
    // TODO: There needs to be some way to handle default focus, eg. so that the editor can be focused when the app starts.
    // Maybe widgets can request being the default in WidgetLayout and when a widget is started if it can be default and the focus list is empty set the
    // focus list to the current stack? This means that the focus wouldn't be correct for the first frame though... maybe we'll need to do one update from App::new to
    // set everything up... that would require some of the platform changes from the other better-ui branch to allow getting window,gfx,etc in App::new.
    focus_stack: Vec<usize>,
    hover_stack: Vec<usize>,
    left_click_position: Option<VisualPosition>,
    id_stack: Vec<usize>,
    current_stack: Vec<usize>,
    container_direction_stack: Vec<ContainerDirection>,
    bounds_stack: Vec<Rect>,
    current_layout: WidgetLayout,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            sibling_focus_history: Vec::new(),
            focus_stack: Vec::new(),
            hover_stack: Vec::new(),
            left_click_position: None,
            id_stack: Vec::new(),
            current_stack: Vec::new(),
            container_direction_stack: Vec::new(),
            bounds_stack: Vec::new(),
            current_layout: WidgetLayout::default(),
        }
    }

    pub fn begin(&mut self, clear_color: Color, window: &mut Window, gfx: &mut Gfx) {
        self.hover_stack.clear();
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

        self.id_stack.push(0);
        self.bounds_stack.push(bounds);
        self.container_direction_stack
            .push(ContainerDirection::Vertical);
        gfx.begin_frame(clear_color);
    }

    pub fn end(&mut self, gfx: &mut Gfx) {
        gfx.end_frame();
        self.container_direction_stack.pop();
        self.bounds_stack.pop();
        self.id_stack.pop();

        if self.left_click_position.is_some() {
            self.left_click_position = None;
            println!("focusing: {:?}", self.hover_stack);

            Self::update_focus_history(
                &self.focus_stack,
                &self.hover_stack,
                &mut self.sibling_focus_history,
            );

            self.focus_stack.clear();
            self.focus_stack.extend_from_slice(&self.hover_stack);
        }
    }

    pub fn begin_container(&mut self, layout: WidgetLayout, direction: ContainerDirection) {
        assert!(!self.is_in_widget());

        self.begin_id();
        self.begin_bounds(layout);
        self.id_stack.push(0);
        self.container_direction_stack.push(direction);
    }

    pub fn end_container(&mut self) {
        assert!(!self.is_in_widget());

        self.container_direction_stack.pop();
        self.id_stack.pop();
        self.end_bounds();
        self.end_id();
    }

    pub fn begin_widget(&mut self, layout: WidgetLayout, gfx: &mut Gfx) {
        assert!(!self.is_in_widget());

        self.begin_id();
        self.begin_bounds(layout);
        gfx.begin(Some(self.bounds()));
    }

    pub fn end_widget(&mut self, gfx: &mut Gfx) {
        assert!(self.is_in_widget());

        gfx.end();
        self.end_bounds();
        self.end_id();
    }

    fn begin_id(&mut self) {
        let id = self.id_stack.last_mut().unwrap();
        self.current_stack.push(*id);
        *id += 1;
    }

    fn end_id(&mut self) {
        self.current_stack.pop();
    }

    // TODO: Remove this.
    pub fn print_current_stack(&self) {
        println!("current stack: {:?}", self.current_stack);
    }
    pub fn print_focus_stack(&self) {
        println!("focus stack: {:?}", self.focus_stack);
    }

    fn begin_bounds(&mut self, layout: WidgetLayout) {
        // TODO: Support choosing if you want to be at the beginning or end of the parent container.
        let container_bounds = self.bounds_stack.last().unwrap();
        let container_direction = self.container_direction_stack.last().unwrap();

        let (x, y) = if let Some(position) = layout.position {
            (position.x, position.y)
        } else {
            (container_bounds.x, container_bounds.y)
        };

        let width = match container_direction {
            ContainerDirection::Horizontal => layout.width.unwrap_or(container_bounds.width),
            ContainerDirection::Vertical => container_bounds.width,
        };

        let height = match container_direction {
            ContainerDirection::Horizontal => container_bounds.height,
            ContainerDirection::Vertical => layout.height.unwrap_or(container_bounds.height),
        };

        let bounds = Rect::new(x, y, width, height);

        if self
            .left_click_position
            .is_some_and(|position| bounds.contains_position(position))
        {
            // println!("pushing hover: {:?}", self.id());
            // self.hover_stack.push(self.id());
            self.hover_stack.clear();
            self.hover_stack.extend_from_slice(&self.current_stack);
        }

        self.current_layout = layout;
        self.bounds_stack.push(bounds);
    }

    fn end_bounds(&mut self) {
        let bounds = self.bounds_stack.pop().unwrap();

        if self.current_layout.position.is_some() {
            return;
        }

        let container_bounds = self.bounds_stack.last_mut().unwrap();
        let container_direction = self.container_direction_stack.last().unwrap();

        *container_bounds = match container_direction {
            ContainerDirection::Horizontal => container_bounds.shrink_left_by(bounds),
            ContainerDirection::Vertical => container_bounds.shrink_top_by(bounds),
        }
    }

    pub fn bounds(&self) -> Rect {
        *self.bounds_stack.last().unwrap()
    }

    pub fn focus(&mut self) {
        if self.focus_stack == self.current_stack || self.left_click_position.is_some() {
            return;
        }

        Self::update_focus_history(
            &self.focus_stack,
            &self.current_stack,
            &mut self.sibling_focus_history,
        );

        self.focus_stack.clear();
        self.focus_stack.extend_from_slice(&self.current_stack);
    }

    fn update_focus_history(
        old_focus_stack: &[usize],
        new_focus_stack: &[usize],
        sibling_focus_history: &mut Vec<usize>,
    ) {
        if old_focus_stack == new_focus_stack {
            return;
        }

        let has_same_parent = old_focus_stack.len() == new_focus_stack.len()
            && old_focus_stack.get(old_focus_stack.len() - 2)
                == new_focus_stack.get(new_focus_stack.len() - 2);

        if has_same_parent {
            let sibling_id = old_focus_stack.last().unwrap();
            sibling_focus_history.push(*sibling_id);
        } else {
            sibling_focus_history.clear();
        }
    }

    // TODO: When a command palette is closed or a pane is removed it needs to be unfocused if it is focused.
    pub fn unfocus(&mut self) {
        if self.focus_stack.is_empty() {
            return;
        }

        if let Some(sibling_id) = self.sibling_focus_history.pop() {
            *self.focus_stack.last_mut().unwrap() = sibling_id;
        } else {
            self.focus_stack.pop();
        }
    }

    pub fn is_focused(&self) -> bool {
        if self.current_stack.len() > self.focus_stack.len() {
            return false;
        }

        self.focus_stack[..self.current_stack.len()] == self.current_stack
    }

    pub fn is_hovered(&self, window: &Window) -> bool {
        self.bounds().contains_position(window.mouse_position())
    }

    fn is_in_widget(&self) -> bool {
        self.bounds_stack.len() > self.container_direction_stack.len()
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
