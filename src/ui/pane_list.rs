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
    widget_list::WidgetList,
};

pub trait PaneWrapper<T>: Deref<Target = Pane<T>> + DerefMut<Target = Pane<T>> {
    fn receive_msgs(&mut self, data_list: &mut SlotList<T>, ctx: &mut Ctx);
    fn update(&mut self, data_list: &mut SlotList<T>, ctx: &mut Ctx);
    fn widget_id(&self) -> WidgetId;
}

pub struct PaneList<TPane: PaneWrapper<TData>, TData> {
    widget_id: WidgetId,
    panes: WidgetList<TPane>,
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
            panes: WidgetList::new(PaneWrapper::widget_id),
            _phantom: PhantomData,
        }
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.panes.iter().any(|pane| pane.is_animating(ctx))
    }

    pub fn receive_msgs(&mut self, data_list: &mut SlotList<TData>, ctx: &mut Ctx) {
        while let Some(msg) = ctx.ui.msg(self.widget_id) {
            match msg {
                Msg::Action(action_name!(PreviousPane)) => self.panes.focus_previous(ctx.ui),
                Msg::Action(action_name!(NextPane)) => self.panes.focus_next(ctx.ui),
                Msg::Action(action_name!(PreviousTab)) => self.panes.focus_previous(ctx.ui),
                Msg::Action(action_name!(NextTab)) => self.panes.focus_next(ctx.ui),
                _ => ctx.ui.skip(self.widget_id, msg),
            }
        }

        for pane in self.panes.iter_mut() {
            pane.receive_msgs(data_list, ctx);
        }
    }

    pub fn update(&mut self, data_list: &mut SlotList<TData>, ctx: &mut Ctx) {
        for pane in self.panes.iter_mut() {
            pane.update(data_list, ctx);
        }

        self.panes.update(ctx.ui);
    }

    pub fn animate(&mut self, data_list: &mut SlotList<TData>, ctx: &mut Ctx, dt: f32) {
        for pane in self.panes.iter_mut() {
            pane.animate(data_list, ctx, dt);
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

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }
}

impl<TPane: PaneWrapper<TData>, TData> Deref for PaneList<TPane, TData> {
    type Target = WidgetList<TPane>;

    fn deref(&self) -> &Self::Target {
        &self.panes
    }
}

impl<TPane: PaneWrapper<TData>, TData> DerefMut for PaneList<TPane, TData> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.panes
    }
}
