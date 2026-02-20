use crate::{
    config::theme::Theme,
    ctx::Ctx,
    geometry::{
        rect::Rect,
        sides::{Side, Sides},
        visual_position::{self, VisualPosition},
    },
    input::{
        action::action_name,
        mods::Mods,
        mouse_button::MouseButton,
        mouse_scroll::MouseScroll,
        mousebind::{Mousebind, MousebindKind},
    },
    text::doc::Doc,
    ui::{
        camera::{CameraAxis, CameraRecenterRequest},
        core::WidgetLayout,
        msg::Msg,
    },
};

use super::{
    color::Color,
    core::{Ui, WidgetId, WidgetSettings},
    focus_list::FocusList,
    slot_list::{SlotId, SlotList},
    tab::Tab,
};

pub struct Pane<T> {
    // TODO: Maybe this could be a widget list.
    tabs: FocusList<Tab>,
    handled_focused_index: Option<usize>,
    dragged_tab_offset: Option<f32>,
    tab_bar_width: f32,
    camera: CameraAxis,

    get_doc: fn(&T) -> &Doc,
    get_doc_mut: fn(&mut T) -> &mut Doc,

    widget_id: WidgetId,
}

impl<T> Pane<T> {
    pub fn new(
        get_doc: fn(&T) -> &Doc,
        get_doc_mut: fn(&mut T) -> &mut Doc,
        parent_id: WidgetId,
        ui: &mut Ui,
    ) -> Self {
        Self {
            tabs: FocusList::new(),
            handled_focused_index: None,
            dragged_tab_offset: None,
            tab_bar_width: 0.0,
            camera: CameraAxis::new(),

            get_doc,
            get_doc_mut,

            widget_id: ui.new_widget(
                parent_id,
                WidgetSettings {
                    layout: WidgetLayout::Tab { index: 0 },
                    ..Default::default()
                },
            ),
        }
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.camera.is_moving() || self.tabs.iter().any(|tab| tab.is_animating(ctx))
    }

    // pub fn layout(&mut self, bounds: Rect, data_list: &mut SlotList<T>, ctx: &mut Ctx) {
    //     ctx.ui.widget_mut(self.widget_id).bounds = bounds;

    //     let gfx = &mut ctx.gfx;

    //     let mut tab_x = 0.0;
    //     let tab_height = gfx.tab_height();

    //     for i in 0..self.tabs.len() {
    //         let get_doc = self.get_doc;

    //         let Some((tab, data)) = self.get_tab_with_data_mut(i, data_list) else {
    //             return;
    //         };

    //         let doc = (get_doc)(data);

    //         let tab_width = gfx.glyph_width() * 4.0
    //             + gfx.measure_text(doc.file_name()) as f32 * gfx.glyph_width();

    //         let tab_bounds = Rect::new(tab_x, bounds.y, tab_width, tab_height);

    //         let doc_bounds = bounds
    //             .shrink_left_by(bounds.left_border(gfx.border_width()))
    //             .shrink_top_by(tab_bounds);

    //         tab_x += tab_width - gfx.border_width();

    //         tab.layout(tab_bounds, doc_bounds, 0.0, doc, gfx);
    //     }

    //     self.tab_bar_width = tab_x;
    // }

    pub fn receive_msgs(&mut self, data_list: &mut SlotList<T>, ctx: &mut Ctx) {
        while let Some(msg) = ctx.ui.msg(self.widget_id) {
            match msg {
                Msg::Mousebind(Mousebind {
                    button: Some(MouseButton::Left),
                    kind: MousebindKind::Release,
                    ..
                })
                | Msg::LostFocus => self.dragged_tab_offset = None,
                Msg::Mousebind(Mousebind {
                    button: Some(MouseButton::Left),
                    mods: Mods::NONE,
                    kind: MousebindKind::Move,
                    ..
                }) => {}
                Msg::Mousebind(Mousebind {
                    button: Some(MouseButton::Left),
                    x,
                    y,
                    mods: Mods::NONE,
                    kind: MousebindKind::Press,
                    ..
                }) => {
                    let bounds = ctx.ui.bounds(self.widget_id);

                    let visual_position =
                        VisualPosition::new(x + self.camera.position(), y).unoffset_by(bounds);

                    let mut offset = 0.0;

                    let index = self.tabs.iter().position(|tab| {
                        let bounds = ctx.ui.bounds(self.widget_id);
                        let tab_bounds = tab.visual_tab_bounds();

                        offset = tab_bounds.x - visual_position.x - bounds.x;

                        tab_bounds.contains_position(visual_position)
                    });

                    if let Some(index) = index {
                        self.tabs.set_focused_index(index);
                        self.handled_focused_index = Some(index);
                        self.dragged_tab_offset = Some(offset);
                    } else {
                        ctx.ui.skip(self.widget_id, msg);
                    }
                }
                Msg::MouseScroll(MouseScroll {
                    x,
                    y,
                    delta,
                    is_horizontal,
                    kind,
                }) => {
                    let bounds = ctx.ui.bounds(self.widget_id);
                    let visual_position = VisualPosition::new(x, y).unoffset_by(bounds);

                    if self
                        .tabs
                        .iter()
                        .any(|tab| tab.tab_bounds().contains_position(visual_position))
                    {
                        ctx.ui.skip(self.widget_id, msg);
                        continue;
                    }

                    let delta =
                        delta * ctx.gfx.glyph_width() * if is_horizontal { 1.0 } else { -1.0 };

                    self.camera.scroll(-delta, kind);
                }
                Msg::Action(action_name!(PreviousTab)) => self.tabs.focus_previous(),
                Msg::Action(action_name!(NextTab)) => self.tabs.focus_next(),
                _ => ctx.ui.skip(self.widget_id, msg),
            }
        }

        for tab in self.tabs.iter_mut() {
            let data = data_list.get_mut(tab.data_id()).unwrap();
            let doc = (self.get_doc_mut)(data);

            tab.receive_msgs(doc, ctx);
        }
    }

    pub fn update(&mut self, ctx: &mut Ctx) {
        self.handle_dragged_tab(ctx.ui);
    }

    fn handle_dragged_tab(&mut self, ui: &mut Ui) {
        if self.dragged_tab_offset.is_none() {
            return;
        }

        let Some(focused_tab) = self.tabs.get_focused() else {
            return;
        };

        let focused_tab_bounds = focused_tab.visual_tab_bounds();
        let focused_tab_right = focused_tab_bounds.right();
        let focused_tab_left = focused_tab_bounds.left();

        let focused_index = self.tabs.focused_index();

        let index = self.tabs.iter().enumerate().position(|(index, tab)| {
            let tab_bounds = tab.tab_bounds();
            let tab_center_x = tab_bounds.center_x();

            if focused_index < index {
                focused_tab_right > tab_center_x && focused_tab_right < tab_bounds.right()
            } else if focused_index > index {
                focused_tab_left < tab_center_x && focused_tab_left > tab_bounds.left()
            } else {
                false
            }
        });

        if let Some(index) = index {
            self.swap_tabs(self.tabs.focused_index(), index, ui);
        }
    }

    pub fn animate(&mut self, data_list: &mut SlotList<T>, ctx: &mut Ctx, dt: f32) {
        self.animate_tab_bar(ctx, dt);

        let focused_index = self.focused_tab_index();
        let widget_id = self.widget_id;
        let get_doc = self.get_doc;

        for (index, tab) in self.tabs.iter_mut().enumerate() {
            let Some(data) = data_list.get_mut(tab.data_id()) else {
                continue;
            };

            let widget_id = (index == focused_index).then_some(widget_id);

            tab.animate(widget_id, get_doc(data), ctx, dt);
        }
    }

    pub fn animate_tab_bar(&mut self, ctx: &mut Ctx, dt: f32) {
        let mut tab = self.tabs.get_focused_mut();

        if let Some((tab, offset)) = tab.as_mut().zip(self.dragged_tab_offset) {
            tab.set_tab_animation_x(
                self.camera.position() + ctx.window.mouse_position().x + offset,
            );
        }

        let recenter_request = self.recenter_request(ctx);

        self.handled_focused_index = Some(self.tabs.focused_index());

        let view_size = ctx.ui.bounds(self.widget_id).width;
        let max_position = (self.tab_bar_width - view_size).max(0.0);

        self.camera
            .animate(recenter_request, max_position, view_size, dt);
    }

    fn recenter_request(&self, ctx: &Ctx) -> CameraRecenterRequest {
        if self.dragged_tab_offset.is_some() {
            self.recenter_request_dragging(ctx)
        } else {
            self.recenter_request_on_tab()
        }
    }

    fn recenter_request_dragging(&self, ctx: &Ctx) -> CameraRecenterRequest {
        let bounds = ctx.ui.bounds(self.widget_id);
        let mouse_position = ctx.window.mouse_position().unoffset_by(bounds);

        CameraRecenterRequest {
            can_start: true,
            target_position: mouse_position.x,
            scroll_border: 0.0,
        }
    }

    fn recenter_request_on_tab(&self) -> CameraRecenterRequest {
        let tab = self.tabs.get_focused();
        let focused_index = self.tabs.focused_index();
        let tab_bounds = tab.map(Tab::tab_bounds).unwrap_or_default();

        CameraRecenterRequest {
            can_start: Some(focused_index) != self.handled_focused_index,
            target_position: tab_bounds.center_x() - self.camera.position(),
            scroll_border: tab_bounds.width,
        }
    }

    pub fn draw(&mut self, background: Option<Color>, data_list: &mut SlotList<T>, ctx: &mut Ctx) {
        self.draw_tab_bar(background, data_list, ctx);

        let get_doc_mut = self.get_doc_mut;
        let is_focused = ctx.ui.is_focused(self.widget_id);

        if let Some((tab, data)) = self.get_tab_with_data_mut(self.tabs.focused_index(), data_list)
        {
            tab.draw((None, background), get_doc_mut(data), ctx);
        }
    }

    fn draw_tab_bar(
        &mut self,
        background: Option<Color>,
        data_list: &mut SlotList<T>,
        ctx: &mut Ctx,
    ) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;
        let bounds = ctx.ui.bounds(self.widget_id);

        gfx.begin(Some(bounds));

        gfx.add_bordered_rect(
            bounds.unoffset_by(bounds),
            [Side::Top, Side::Left].into(),
            theme.background,
            theme.border,
        );

        for i in 0..self.tabs.len() {
            if i == self.tabs.focused_index() {
                continue;
            }

            self.draw_tab_from_index(i, background, data_list, ctx);
        }

        let focused_tab_bounds =
            self.draw_tab_from_index(self.tabs.focused_index(), background, data_list, ctx);

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;
        let tab_height = gfx.tab_height();

        gfx.add_rect(
            Rect::from_sides(
                0.0,
                tab_height - gfx.border_width(),
                focused_tab_bounds.x,
                tab_height,
            ),
            theme.border,
        );

        gfx.add_rect(
            Rect::from_sides(
                focused_tab_bounds.x + focused_tab_bounds.width,
                tab_height - gfx.border_width(),
                bounds.width,
                tab_height,
            ),
            theme.border,
        );

        gfx.end();
    }

    fn draw_tab_from_index(
        &self,
        index: usize,
        background: Option<Color>,
        data_list: &SlotList<T>,
        ctx: &mut Ctx,
    ) -> Rect {
        let get_doc = self.get_doc;

        let Some((tab, data)) = self.get_tab_with_data(index, data_list) else {
            return Rect::ZERO;
        };

        let is_focused = index == self.tabs.focused_index();

        self.draw_tab(is_focused, background, tab, get_doc(data), ctx)
    }

    fn draw_tab(
        &self,
        is_focused: bool,
        background: Option<Color>,
        tab: &Tab,
        doc: &Doc,
        ctx: &mut Ctx,
    ) -> Rect {
        let theme = &ctx.config.theme;

        let text_color = Self::tab_color(doc, theme, ctx);

        let camera_x = self.camera.position().floor();

        let tab_bounds = tab.visual_tab_bounds().shift_x(-camera_x);
        let mut tab_background = theme.background;

        if is_focused {
            if let Some(background) = background {
                tab_background = background;
            }
        }

        let gfx = &mut ctx.gfx;

        gfx.add_bordered_rect(
            tab_bounds,
            Sides::ALL.without(Side::Bottom),
            tab_background,
            theme.border,
        );

        let text_x = (tab_bounds.x + gfx.glyph_width() * 2.0).floor();
        let text_y = gfx.border_width() + gfx.tab_padding_y();
        let text_width = gfx.add_text(doc.file_name(), text_x, text_y, text_color);

        if !doc.is_saved() {
            gfx.add_text("*", text_x + text_width, text_y, theme.symbol);
        }

        if is_focused {
            let foreground = if ctx.ui.is_focused(self.widget_id) {
                theme.keyword
            } else {
                theme.emphasized
            };

            gfx.add_rect(tab_bounds.top_border(gfx.border_width()), foreground);
        }

        tab_bounds
    }

    fn tab_color(doc: &Doc, theme: &Theme, ctx: &mut Ctx) -> Color {
        for language_server in ctx.lsp.iter_servers_mut() {
            for diagnostic in language_server.diagnostics_mut(doc) {
                if !diagnostic.is_problem() {
                    continue;
                }

                return diagnostic.color(theme);
            }
        }

        theme.normal
    }

    pub fn get_tab_with_data_mut<'a>(
        &'a mut self,
        tab_index: usize,
        data_list: &'a mut SlotList<T>,
    ) -> Option<(&'a mut Tab, &'a mut T)> {
        let tab = self.tabs.get_mut(tab_index)?;
        let data = data_list.get_mut(tab.data_id())?;

        Some((tab, data))
    }

    pub fn get_tab_with_data<'a>(
        &'a self,
        tab_index: usize,
        data_list: &'a SlotList<T>,
    ) -> Option<(&'a Tab, &'a T)> {
        let tab = self.tabs.get(tab_index)?;
        let data = data_list.get(tab.data_id())?;

        Some((tab, data))
    }

    pub fn get_focused_tab_with_data_mut<'a>(
        &'a mut self,
        data_list: &'a mut SlotList<T>,
    ) -> Option<(&'a mut Tab, &'a mut T)> {
        self.get_tab_with_data_mut(self.focused_tab_index(), data_list)
    }

    pub fn get_focused_tab_with_data<'a>(
        &'a self,
        data_list: &'a SlotList<T>,
    ) -> Option<(&'a Tab, &'a T)> {
        self.get_tab_with_data(self.focused_tab_index(), data_list)
    }

    pub fn get_existing_tab_for_data(&self, data_id: SlotId) -> Option<usize> {
        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.data_id() == data_id {
                return Some(i);
            }
        }

        None
    }

    pub fn add_tab(&mut self, data_id: SlotId, data_list: &mut SlotList<T>, ctx: &mut Ctx) {
        let tab = Tab::new(self.widget_id, data_id, ctx.ui);

        data_list
            .get_mut(data_id)
            .map(|data| (self.get_doc_mut)(data))
            .expect("tried to add a tab referencing a non-existent data")
            .add_usage();

        self.tabs.add(tab);
        self.set_layout(ctx.ui);

        let bounds = ctx.ui.bounds(self.widget_id);
        // TODO:
        // self.layout(bounds, data_list, ctx);
    }

    pub fn remove_tab(&mut self, data_list: &mut SlotList<T>, ui: &mut Ui) {
        let get_doc_mut = self.get_doc_mut;
        let index = self.tabs.focused_index();

        let Some((_, data)) = self.get_tab_with_data_mut(index, data_list) else {
            return;
        };

        (get_doc_mut)(data).remove_usage();

        self.tabs.remove();

        if let Some(child_id) = ui.child_ids(self.widget_id).get(index) {
            ui.remove_widget(*child_id);
        }

        self.set_layout(ui);
    }

    fn swap_tabs(&mut self, a: usize, b: usize, ui: &mut Ui) {
        self.tabs.swap(a, b);
        self.set_layout(ui);
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }

    pub fn iter_tabs(&self) -> impl Iterator<Item = &Tab> {
        self.tabs.iter()
    }

    pub fn iter_tabs_mut(&mut self) -> impl Iterator<Item = &mut Tab> {
        self.tabs.iter_mut()
    }

    pub fn get_focused_tab(&self) -> Option<&Tab> {
        self.tabs.get_focused()
    }

    pub fn set_focused_tab_index(&mut self, index: usize, ui: &mut Ui) {
        self.tabs.set_focused_index(index);
        self.set_layout(ui);
    }

    pub fn focused_tab_index(&self) -> usize {
        self.tabs.focused_index()
    }

    fn set_layout(&mut self, ui: &mut Ui) {
        ui.set_layout(
            self.widget_id,
            WidgetLayout::Tab {
                index: self.focused_tab_index(),
            },
        );

        if let Some(child_id) = ui.child_ids(self.widget_id).get(self.focused_tab_index()) {
            ui.focus(*child_id);
        }
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }
}
