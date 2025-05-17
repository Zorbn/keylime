use crate::pool::{Pooled, VEC_WIDGET_ID_POOL};

use super::core::{Ui, WidgetId};

pub struct WidgetList<T> {
    widgets: Vec<T>,
    // The stack representing the widgets in the list.
    widget_stack: Pooled<Vec<WidgetId>>,
    // A sub stack representing the child of these widgets to focus.
    child_stack: Pooled<Vec<WidgetId>>,
    last_focused_index: usize,
}

impl<T> WidgetList<T> {
    pub fn new(widget_stack: &[WidgetId], child_stack: &[WidgetId]) -> Self {
        Self {
            widgets: Vec::new(),
            widget_stack: VEC_WIDGET_ID_POOL
                .init_item(|stack| stack.extend_from_slice(widget_stack)),
            child_stack: VEC_WIDGET_ID_POOL.init_item(|stack| stack.extend_from_slice(child_stack)),
            last_focused_index: 0,
        }
    }

    pub fn update(&mut self, ui: &Ui) {
        self.last_focused_index = self.last_focused_index(ui);
    }

    pub fn focus_next(&mut self, ui: &mut Ui) {
        let last_focused_index = self.last_focused_index(ui);

        if last_focused_index < self.widgets.len().saturating_sub(1) {
            self.focus_index(last_focused_index + 1, ui);
        }
    }

    pub fn focus_previous(&mut self, ui: &mut Ui) {
        let last_focused_index = self.last_focused_index(ui);

        if last_focused_index > 0 {
            self.focus_index(last_focused_index - 1, ui);
        }
    }

    fn focus_index(&mut self, index: usize, ui: &mut Ui) {
        // let widget = &self.widgets[index];

        // let get_widget_id = self.get_widget_id;
        // ui.focus(get_widget_id(widget));

        let mut stack = self.widget_stack_with_index(index);
        stack.extend_from_slice(&self.child_stack);
        ui.focus(&stack);

        self.last_focused_index = index;
    }

    pub fn add(&mut self, widget: T, ui: &mut Ui) {
        // let get_widget_id = self.get_widget_id;
        // ui.focus(get_widget_id(&widget));

        let last_focused_index = self.last_focused_index(ui);
        let focused_index;

        if last_focused_index >= self.len() {
            focused_index = self.len();

            self.widgets.push(widget);
        } else {
            focused_index = last_focused_index + 1;

            self.widgets.insert(last_focused_index + 1, widget);
        }

        self.focus_index(focused_index, ui);
    }

    pub fn remove(&mut self, ui: &mut Ui) {
        let last_focused_index = self.last_focused_index(ui);

        if last_focused_index < self.len() {
            ui.unfocus(&self.widget_stack_with_index(last_focused_index));
            self.widgets.remove(last_focused_index);

            self.clamp_last_focused_index();
        }
    }

    pub fn len(&self) -> usize {
        self.widgets.len()
    }

    pub fn last_focused_index(&self, ui: &Ui) -> usize {
        for i in 0..self.widgets.len() {
            let stack = self.widget_stack_with_index(i);

            if ui.is_stack_focused(&stack) {
                return i;
            }
        }

        self.last_focused_index

        // let index = self
        //     .widgets
        //     .iter()
        //     .position(|widget| ui.is_focused(get_widget_id(widget)))
        //     .unwrap_or(self.last_focused_index);

        // index
    }

    pub fn get_last_focused(&self, ui: &Ui) -> Option<&T> {
        let index = self.last_focused_index(ui);
        self.widgets.get(index)
    }

    pub fn get_last_focused_mut(&mut self, ui: &Ui) -> Option<&mut T> {
        let index = self.last_focused_index(ui);
        self.widgets.get_mut(index)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.widgets.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.widgets.iter_mut()
    }

    fn clamp_last_focused_index(&mut self) {
        self.last_focused_index = self
            .last_focused_index
            .min(self.widgets.len().saturating_sub(1))
    }

    fn widget_stack_with_index(&self, index: usize) -> Pooled<Vec<WidgetId>> {
        let mut stack = self.widget_stack.clone();

        if let Some(last) = stack.last_mut() {
            *last = WidgetId::NameWithIndex(last.name(), index);
        }

        stack
    }
}
