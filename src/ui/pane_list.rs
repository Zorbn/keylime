use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{
    ctx::Ctx,
    input::action::action_name,
    ui::{
        core::{Ui, WidgetSettings},
        msg::Msg,
    },
};

use super::{
    color::Color,
    core::{WidgetId, WidgetLayout},
    pane::Pane,
    slot_list::SlotList,
};

pub trait PaneWrapper<T>: Deref<Target = Pane<T>> + DerefMut<Target = Pane<T>> {
    fn receive_msgs(&mut self, data_list: &mut SlotList<T>, ctx: &mut Ctx);
    fn update(&mut self, data_list: &mut SlotList<T>, ctx: &mut Ctx, dt: f32);
    fn widget_id(&self) -> WidgetId;
}

pub struct PaneList<TPane: PaneWrapper<TData>, TData> {
    widget_id: WidgetId,
    panes: Vec<TPane>,
    last_focused_child_index: usize,
    _phantom: PhantomData<TData>,
}

impl<TPane: PaneWrapper<TData>, TData> PaneList<TPane, TData> {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        Self {
            widget_id: ui.new_widget(
                parent_id,
                WidgetSettings {
                    layout: WidgetLayout::Horizontal,
                    ..Default::default()
                },
            ),
            panes: Vec::new(),
            last_focused_child_index: 0,
            _phantom: PhantomData,
        }
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.panes.iter().any(|pane| pane.is_animating(ctx))
    }

    pub fn receive_msgs(&mut self, data_list: &mut SlotList<TData>, ctx: &mut Ctx) {
        while let Some(msg) = ctx.ui.msg(self.widget_id) {
            match msg {
                Msg::FocusedChildChanged | Msg::GainedFocus => {
                    if let Some(focused_child_index) = ctx
                        .ui
                        .child_ids(self.widget_id)
                        .iter()
                        .position(|child_id| ctx.ui.is_focused(*child_id))
                    {
                        self.last_focused_child_index = focused_child_index;
                    }
                }
                Msg::Action(action_name!(PreviousPane)) => self.focus_previous(ctx.ui),
                Msg::Action(action_name!(NextPane)) => self.focus_next(ctx.ui),
                Msg::Action(action_name!(PreviousTab)) => self.focus_previous(ctx.ui),
                Msg::Action(action_name!(NextTab)) => self.focus_next(ctx.ui),
                _ => ctx.ui.skip(self.widget_id, msg),
            }
        }

        for pane in self.panes.iter_mut() {
            pane.receive_msgs(data_list, ctx);
        }
    }

    pub fn update(&mut self, data_list: &mut SlotList<TData>, ctx: &mut Ctx, dt: f32) {
        for pane in self.panes.iter_mut() {
            pane.update(data_list, ctx, dt);
        }
    }

    pub fn draw(
        &mut self,
        background: Option<Color>,
        data_list: &mut SlotList<TData>,
        ctx: &mut Ctx,
    ) {
        for pane in self.panes.iter_mut() {
            pane.draw(background, data_list, ctx);
        }
    }

    pub fn get_last_focused(&self, ui: &Ui) -> Option<&TPane> {
        self.get_last_focused_index(ui)
            .and_then(|index| self.panes.get(index))
    }

    pub fn get_last_focused_mut(&mut self, ui: &Ui) -> Option<&mut TPane> {
        self.get_last_focused_index(ui)
            .and_then(|index| self.panes.get_mut(index))
    }

    fn get_last_focused_index(&self, ui: &Ui) -> Option<usize> {
        ui.child_ids(self.widget_id)
            .get(self.last_focused_child_index)
            .and_then(|child_id| {
                self.panes
                    .iter()
                    .position(|pane| pane.widget_id() == *child_id)
            })
    }

    pub fn get_hovered_mut(&mut self, ui: &Ui) -> Option<&mut TPane> {
        self.panes
            .iter()
            .position(|pane| ui.is_hovered(pane.widget_id()))
            .and_then(|index| self.panes.get_mut(index))
    }

    pub fn add(&mut self, pane: TPane, ui: &mut Ui) {
        if self.last_focused_child_index + 1 < self.panes.len() {
            ui.move_child(pane.widget_id(), self.last_focused_child_index + 1);
        }

        ui.focus(pane.widget_id());
        self.panes.push(pane);
    }

    pub fn remove_focused(&mut self, ui: &mut Ui) {
        let Some(focused_id) = ui
            .child_ids(self.widget_id)
            .get(self.last_focused_child_index)
            .copied()
        else {
            return;
        };

        self.remove(focused_id, ui);
    }

    pub fn remove(&mut self, widget_id: WidgetId, ui: &mut Ui) {
        ui.remove_widget(widget_id);

        if let Some(index) = self
            .panes
            .iter()
            .position(|pane| pane.widget_id() == widget_id)
        {
            self.panes.remove(index);
        }

        let child_ids = ui.child_ids(self.widget_id);

        if child_ids.is_empty() {
            ui.focus(self.widget_id);
            return;
        }

        let index = self.last_focused_child_index.min(child_ids.len() - 1);
        ui.focus(child_ids[index]);
    }

    pub fn remove_excess(&mut self, ui: &mut Ui, predicate: impl Fn(&TPane) -> bool) {
        for i in (0..self.panes.len()).rev() {
            if self.panes.len() == 1 {
                break;
            }

            if predicate(&self.panes[i]) {
                self.remove(self.panes[i].widget_id(), ui);
            }
        }

        self.last_focused_child_index = self
            .last_focused_child_index
            .min(self.panes.len().saturating_sub(1))
    }

    fn focus_next(&self, ui: &mut Ui) {
        let child_ids = ui.child_ids(self.widget_id);
        let index = self.last_focused_child_index + 1;

        if index < child_ids.len() {
            ui.focus(child_ids[index]);
        }
    }

    fn focus_previous(&self, ui: &mut Ui) {
        let child_ids = ui.child_ids(self.widget_id);

        if self.last_focused_child_index > 0 {
            ui.focus(child_ids[self.last_focused_child_index - 1]);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &TPane> {
        self.panes.iter()
    }

    pub fn len(&self) -> usize {
        self.panes.len()
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }
}
