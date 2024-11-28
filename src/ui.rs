use widget::Widget;

use crate::{
    geometry::visual_position::VisualPosition,
    input::{mouse_button::MouseButton, mousebind::Mousebind},
    platform::{gfx::Gfx, window::Window},
};

mod camera;
pub mod color;
pub mod command_palette;
mod doc_list;
pub mod editor;
mod editor_pane;
mod pane;
mod result_list;
pub mod tab;
pub mod terminal;
pub mod terminal_emulator;
mod terminal_pane;
pub mod widget;

pub struct Ui {
    next_widget_id: usize,
    focused_widget_id: usize,
    last_focused_widget_id: usize,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            next_widget_id: 0,
            focused_widget_id: 0,
            last_focused_widget_id: 0,
        }
    }

    pub fn get_handle<'a>(&'a mut self, window: &'a mut Window) -> UiHandle<'a> {
        UiHandle::new(self, window)
    }
}

pub struct UiHandle<'a> {
    inner: &'a mut Ui,
    window: &'a mut Window,
}

impl<'a> UiHandle<'a> {
    pub fn new(ui: &'a mut Ui, window: &'a mut Window) -> Self {
        Self { inner: ui, window }
    }

    pub fn gfx(&mut self) -> &mut Gfx {
        self.window.gfx()
    }

    pub fn update(&mut self, focusable_widgets: &mut [&mut Widget]) {
        let mut focused_widget_index = None;

        let mut mousebind_handler = self.window.get_mousebind_handler();

        while let Some(mousebind) = mousebind_handler.next(self.window) {
            if let Mousebind {
                button: Some(MouseButton::Left),
                x,
                y,
                is_drag: false,
                ..
            } = mousebind
            {
                for (index, widget) in focusable_widgets.iter().enumerate() {
                    if !widget.is_visible {
                        continue;
                    }

                    for bounds in widget.all_bounds() {
                        if bounds.contains_position(VisualPosition::new(x, y)) {
                            focused_widget_index = Some(index);

                            break;
                        }
                    }
                }
            }

            mousebind_handler.unprocessed(self.window, mousebind);
        }

        if let Some(focused_widget_index) = focused_widget_index {
            focusable_widgets[focused_widget_index].take_focus(self);
        }
    }
}
