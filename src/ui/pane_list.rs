use std::ops::{Deref, DerefMut};

use crate::{
    ctx::Ctx,
    geometry::rect::Rect,
    input::{
        action::{action_name, Action},
        input_handlers::KeybindHandler,
    },
};

use super::{
    color::Color, core::WidgetId, pane::Pane, slot_list::SlotList, widget_list::WidgetList,
};

pub struct PaneList<TPane: Deref<Target = Pane<TData>> + DerefMut<Target = Pane<TData>>, TData> {
    panes: WidgetList<TPane>,
}

impl<TPane: Deref<Target = Pane<TData>> + DerefMut<Target = Pane<TData>>, TData>
    PaneList<TPane, TData>
{
    pub fn new() -> Self {
        Self {
            panes: WidgetList::new(|pane| pane.widget_id()),
        }
    }

    pub fn is_animating(&self) -> bool {
        self.panes.iter().any(|pane| pane.is_animating())
    }

    pub fn layout(&mut self, bounds: Rect, data_list: &mut SlotList<TData>, ctx: &mut Ctx) {
        let mut pane_bounds = bounds;
        pane_bounds.width = (pane_bounds.width / self.panes.len() as f32).ceil();

        for pane in self.panes.iter_mut() {
            pane.layout(pane_bounds, data_list, ctx);
            pane_bounds.x += pane_bounds.width;
        }
    }

    pub fn update(&mut self, widget_id: WidgetId, ctx: &mut Ctx) {
        self.panes.update(ctx.ui);

        let mut keybind_handler = ctx.ui.keybind_handler(widget_id, ctx.window);

        while let Some(action) = keybind_handler.next_action(ctx) {
            match action {
                action_name!(PreviousPane) => self.panes.focus_previous(ctx.ui),
                action_name!(NextPane) => self.panes.focus_next(ctx.ui),
                action_name!(PreviousTab) => self.previous_tab(action, &mut keybind_handler, ctx),
                action_name!(NextTab) => self.next_tab(action, &mut keybind_handler, ctx),
                _ => keybind_handler.unprocessed(ctx.window, action.keybind),
            }
        }
    }

    pub fn update_camera(&mut self, data_list: &mut SlotList<TData>, ctx: &mut Ctx, dt: f32) {
        for pane in self.panes.iter_mut() {
            pane.update_camera(data_list, ctx, dt);
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

    fn previous_tab(
        &mut self,
        action: Action,
        keybind_handler: &mut KeybindHandler,
        ctx: &mut Ctx,
    ) {
        let Some(pane) = self.panes.get_last_focused(ctx.ui) else {
            return;
        };

        if pane.focused_tab_index() == 0 {
            self.panes.focus_previous(ctx.ui);
        } else {
            keybind_handler.unprocessed(ctx.window, action.keybind);
        }
    }

    fn next_tab(&mut self, action: Action, keybind_handler: &mut KeybindHandler, ctx: &mut Ctx) {
        let Some(pane) = self.panes.get_last_focused(ctx.ui) else {
            return;
        };

        if pane.focused_tab_index() == pane.tabs.len() - 1 {
            self.panes.focus_next(ctx.ui);
        } else {
            keybind_handler.unprocessed(ctx.window, action.keybind);
        }
    }
}

impl<TPane: Deref<Target = Pane<TData>> + DerefMut<Target = Pane<TData>>, TData> Deref
    for PaneList<TPane, TData>
{
    type Target = WidgetList<TPane>;

    fn deref(&self) -> &Self::Target {
        &self.panes
    }
}

impl<TPane: Deref<Target = Pane<TData>> + DerefMut<Target = Pane<TData>>, TData> DerefMut
    for PaneList<TPane, TData>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.panes
    }
}
