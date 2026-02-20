use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{
    ctx::Ctx,
    input::{
        action::{action_name, Action},
        input_handlers::ActionHandler,
    },
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
}

// impl<TPane: Deref<Target = Pane<TData>> + DerefMut<Target = Pane<TData>>, TData> PaneWrapper<TData>
//     for TPane
// {
// }

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
            panes: WidgetList::new(|pane| pane.widget_id()),
            _phantom: PhantomData,
        }
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.panes.iter().any(|pane| pane.is_animating(ctx))
    }

    // pub fn layout(&mut self, bounds: Rect, data_list: &mut SlotList<TData>, ctx: &mut Ctx) {
    //     let mut pane_bounds = bounds;
    //     pane_bounds.width = (pane_bounds.width / self.panes.len() as f32).ceil();

    //     for pane in self.panes.iter_mut() {
    //         pane.layout(pane_bounds, data_list, ctx);
    //         pane_bounds.x += pane_bounds.width;
    //     }
    // }

    pub fn receive_msgs(&mut self, data_list: &mut SlotList<TData>, ctx: &mut Ctx) {
        while let Some(msg) = ctx.ui.msg(self.widget_id) {
            match msg {
                Msg::Action(action_name!(PreviousPane)) => self.panes.focus_previous(ctx.ui),
                Msg::Action(action_name!(NextPane)) => self.panes.focus_next(ctx.ui),
                Msg::Action(action_name!(PreviousTab)) => {
                    if !self.previous_tab(ctx) {
                        ctx.ui.skip(self.widget_id, msg);
                    }
                }
                Msg::Action(action_name!(NextTab)) => {
                    if !self.next_tab(ctx) {
                        ctx.ui.skip(self.widget_id, msg);
                    }
                }
                _ => ctx.ui.skip(self.widget_id, msg),
            }
        }

        for pane in self.panes.iter_mut() {
            pane.receive_msgs(data_list, ctx);
        }
    }

    pub fn update(&mut self, widget_id: WidgetId, ctx: &mut Ctx) {
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

    fn previous_tab(&mut self, ctx: &mut Ctx) -> bool {
        let Some(pane) = self.panes.get_last_focused(ctx.ui) else {
            return true;
        };

        if pane.focused_tab_index() == 0 {
            self.panes.focus_previous(ctx.ui);
            true
        } else {
            false
        }
    }

    fn next_tab(&mut self, ctx: &mut Ctx) -> bool {
        let Some(pane) = self.panes.get_last_focused(ctx.ui) else {
            return true;
        };

        if pane.focused_tab_index() == pane.tab_count() - 1 {
            self.panes.focus_next(ctx.ui);
            true
        } else {
            false
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
