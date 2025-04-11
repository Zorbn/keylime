use widget::Widget;

use crate::{
    geometry::visual_position::VisualPosition,
    input::{mouse_button::MouseButton, mousebind::Mousebind},
    platform::window::Window,
};

mod camera;
pub mod color;
pub mod command_palette;
pub mod editor;
mod pane;
mod result_list;
mod slot_list;
pub mod tab;
pub mod terminal;
pub mod widget;

pub struct Ui {
    next_widget_id: usize,
    focused_widget_id: usize,
    hovered_widget_id: usize,
    last_focused_widget_id: usize,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            next_widget_id: 0,
            focused_widget_id: 0,
            hovered_widget_id: 0,
            last_focused_widget_id: 0,
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
            focusable_widgets[focused_widget_index].take_focus(self);
        }

        if let Some(hovered_widget_index) = hovered_widget_index {
            focusable_widgets[hovered_widget_index].take_hover(self);
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
}
