use std::marker::PhantomData;

use crate::{
    config::theme::Theme,
    ctx::Ctx,
    geometry::{
        rect::Rect,
        sides::{Side, Sides},
        visual_position::VisualPosition,
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
    slot_list::{SlotId, SlotList},
    tab::Tab,
};

struct TabAnimationState {
    target: Rect,
    x: f32,
}

impl TabAnimationState {
    fn visual_bounds(&self) -> Rect {
        Rect {
            x: self.x,
            ..self.target
        }
    }

    fn bounds(&self) -> Rect {
        self.target
    }
}

const TAB_ANIMATION_SPEED: f32 = 10.0;

struct PaneTabBar<T> {
    _phantom: PhantomData<T>,

    tab_animation_states: Vec<TabAnimationState>,
    handled_focused_index: Option<usize>,
    dragged_tab_offset: Option<f32>,
    tab_bar_width: f32,
    camera: CameraAxis,

    widget_id: WidgetId,
}

impl<T> PaneTabBar<T> {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        Self {
            _phantom: PhantomData,

            tab_animation_states: Vec::new(),
            handled_focused_index: None,
            dragged_tab_offset: None,
            tab_bar_width: 0.0,
            camera: CameraAxis::new(),

            widget_id: ui.new_widget(parent_id, Default::default()),
        }
    }

    pub fn is_animating(&self) -> bool {
        self.camera.is_moving()
            || self
                .tab_animation_states
                .iter()
                .any(|tab| (tab.target.x - tab.x).abs() > 0.5)
    }

    pub fn receive_msgs(&mut self, view: &PaneView, ctx: &mut Ctx) {
        while let Some(msg) = ctx.ui.msg(self.widget_id) {
            match msg {
                Msg::Mousebind(Mousebind {
                    button: Some(MouseButton::Left),
                    kind: MousebindKind::Release,
                    ..
                })
                | Msg::LostFocus => {
                    self.dragged_tab_offset = None;
                }
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

                    let index = self.tab_animation_states.iter().position(|tab| {
                        let bounds = ctx.ui.bounds(self.widget_id);
                        let tab_bounds = tab.visual_bounds();

                        offset = tab_bounds.x - visual_position.x - bounds.x;

                        tab_bounds.contains_position(visual_position)
                    });

                    if let Some(index) = index {
                        view.set_focused_index(index, ctx.ui);
                        self.handled_focused_index = Some(index);
                        self.dragged_tab_offset = Some(offset);
                    } else {
                        ctx.ui.skip(self.widget_id, msg);
                    }
                }
                Msg::MouseScroll(MouseScroll {
                    delta,
                    is_horizontal,
                    kind,
                    ..
                }) => {
                    let delta =
                        delta * ctx.gfx.glyph_width() * if is_horizontal { 1.0 } else { -1.0 };

                    self.camera.scroll(-delta, kind);
                }
                _ => ctx.ui.skip(self.widget_id, msg),
            }
        }
    }

    pub fn animate(
        &mut self,
        tabs: &[Tab],
        data_list: &SlotList<T>,
        get_doc: fn(&T) -> &Doc,
        view: &PaneView,
        ctx: &mut Ctx,
        dt: f32,
    ) {
        let gfx = &mut ctx.gfx;

        let mut tab_x = 0.0;
        let tab_height = gfx.tab_height();

        for (index, tab) in tabs.iter().enumerate() {
            let Some(data) = data_list.get(tab.data_id()) else {
                continue;
            };

            let doc = get_doc(data);

            let tab_width = gfx.glyph_width() * 4.0
                + gfx.measure_text(doc.file_name()) as f32 * gfx.glyph_width();

            let target = Rect::new(tab_x, 0.0, tab_width, tab_height);
            let animation_state = &mut self.tab_animation_states[index];

            animation_state.target = target;
            animation_state.x += (target.x - animation_state.x) * TAB_ANIMATION_SPEED * dt;

            tab_x += tab_width - gfx.border_width();
        }

        let focused_index = view.focused_index(ctx.ui);

        if let Some(offset) = self.dragged_tab_offset {
            let tab = &mut self.tab_animation_states[focused_index];
            tab.x = self.camera.position() + ctx.window.mouse_position().x + offset;
        }

        let recenter_request = self.recenter_request(view, ctx);

        self.handled_focused_index = Some(focused_index);

        let view_size = ctx.ui.bounds(self.widget_id).width;
        let max_position = (self.tab_bar_width - view_size).max(0.0);

        self.camera
            .animate(recenter_request, max_position, view_size, dt);
    }

    fn recenter_request(&self, view: &PaneView, ctx: &Ctx) -> CameraRecenterRequest {
        if self.dragged_tab_offset.is_some() {
            self.recenter_request_dragging(ctx)
        } else {
            self.recenter_request_on_tab(view, ctx.ui)
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

    fn recenter_request_on_tab(&self, view: &PaneView, ui: &Ui) -> CameraRecenterRequest {
        let focused_index = view.focused_index(ui);
        let tab_bounds = self.tab_animation_states[focused_index].visual_bounds();

        CameraRecenterRequest {
            can_start: Some(focused_index) != self.handled_focused_index,
            target_position: tab_bounds.center_x() - self.camera.position(),
            scroll_border: tab_bounds.width,
        }
    }

    pub fn update(&mut self, tabs: &mut [Tab], view: &PaneView, ui: &mut Ui) {
        self.handle_dragged_tab(tabs, view, ui);
    }

    // TODO: This should possibly be part of the mouse move move event instead of update.
    fn handle_dragged_tab(&mut self, tabs: &mut [Tab], view: &PaneView, ui: &mut Ui) {
        if self.dragged_tab_offset.is_none() {
            return;
        }

        let focused_index = view.focused_index(ui);
        let focused_tab_bounds = self.tab_animation_states[focused_index].visual_bounds();

        let focused_tab_right = focused_tab_bounds.right();
        let focused_tab_left = focused_tab_bounds.left();

        let index = self
            .tab_animation_states
            .iter()
            .enumerate()
            .position(|(index, tab)| {
                let tab_bounds = tab.bounds();
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
            let focused_widget_id = ui.child_ids(view.widget_id)[focused_index];
            ui.move_child(focused_widget_id, index);
            view.set_focused_index(index, ui);

            tabs.swap(focused_index, index);
            self.tab_animation_states.swap(focused_index, index);
        }
    }

    fn draw(
        &self,
        focused_index: usize,
        is_pane_focused: bool,
        background: Option<Color>,
        tabs: &[Tab],
        data_list: &SlotList<T>,
        get_doc: fn(&T) -> &Doc,
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

        for i in 0..tabs.len() {
            if i == focused_index {
                continue;
            }

            self.draw_tab_from_index(
                i,
                false,
                is_pane_focused,
                background,
                tabs,
                data_list,
                get_doc,
                ctx,
            );
        }

        let focused_tab_bounds = self.draw_tab_from_index(
            focused_index,
            true,
            is_pane_focused,
            background,
            tabs,
            data_list,
            get_doc,
            ctx,
        );

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
        is_focused: bool,
        is_pane_focused: bool,
        background: Option<Color>,
        tabs: &[Tab],
        data_list: &SlotList<T>,
        get_doc: fn(&T) -> &Doc,
        ctx: &mut Ctx,
    ) -> Rect {
        let Some(data) = tabs.get(index).and_then(|tab| data_list.get(tab.data_id())) else {
            return Rect::ZERO;
        };

        let bounds = self.tab_animation_states[index].visual_bounds();

        self.draw_tab(
            is_focused,
            is_pane_focused,
            background,
            bounds,
            get_doc(data),
            ctx,
        )
    }

    fn draw_tab(
        &self,
        is_focused: bool,
        is_pane_focused: bool,
        background: Option<Color>,
        bounds: Rect,
        doc: &Doc,
        ctx: &mut Ctx,
    ) -> Rect {
        let theme = &ctx.config.theme;

        let text_color = Self::tab_color(doc, theme, ctx);

        let camera_x = self.camera.position().floor();

        let bounds = bounds.shift_x(-camera_x);

        let background = if is_focused {
            background.unwrap_or(theme.background)
        } else {
            theme.background
        };

        let gfx = &mut ctx.gfx;

        gfx.add_bordered_rect(
            bounds,
            Sides::ALL.without(Side::Bottom),
            background,
            theme.border,
        );

        let text_x = (bounds.x + gfx.glyph_width() * 2.0).floor();
        let text_y = gfx.border_width() + gfx.tab_padding_y();
        let text_width = gfx.add_text(doc.file_name(), text_x, text_y, text_color);

        if !doc.is_saved() {
            gfx.add_text("*", text_x + text_width, text_y, theme.symbol);
        }

        if is_focused {
            let foreground = if is_pane_focused {
                theme.keyword
            } else {
                theme.emphasized
            };

            gfx.add_rect(bounds.top_border(gfx.border_width()), foreground);
        }

        bounds
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
}

struct PaneView {
    widget_id: WidgetId,
}

impl PaneView {
    fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        Self {
            widget_id: ui.new_widget(
                parent_id,
                WidgetSettings {
                    layout: WidgetLayout::Tab { index: 0 },
                    wants_msgs: false,
                    ..Default::default()
                },
            ),
        }
    }

    fn focused_index(&self, ui: &Ui) -> usize {
        let WidgetLayout::Tab { index } = ui.layout(self.widget_id) else {
            panic!("PaneView should always use a tab layout");
        };

        index
    }

    fn set_focused_index(&self, index: usize, ui: &mut Ui) {
        ui.set_layout(self.widget_id, WidgetLayout::Tab { index });
    }

    fn focus(&self, ui: &mut Ui) {
        let focused_index = self.focused_index(ui);

        let focused_id = ui
            .child_ids(self.widget_id)
            .get(focused_index)
            .copied()
            .unwrap_or(self.widget_id);

        ui.focus(focused_id);
    }
}

pub struct Pane<T> {
    tabs: Vec<Tab>,
    tab_bar: PaneTabBar<T>,
    view: PaneView,

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
        let widget_id = ui.new_widget(
            parent_id,
            WidgetSettings {
                layout: WidgetLayout::Vertical,
                is_resizable: false,
                main_child_index: Some(1),
                ..Default::default()
            },
        );

        Self {
            tabs: Vec::new(),
            tab_bar: PaneTabBar::new(widget_id, ui),
            view: PaneView::new(widget_id, ui),

            get_doc,
            get_doc_mut,

            widget_id,
        }
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.tab_bar.is_animating() || self.tabs.iter().any(|tab| tab.is_animating(ctx))
    }

    pub fn receive_msgs(&mut self, data_list: &mut SlotList<T>, ctx: &mut Ctx) {
        while let Some(msg) = ctx.ui.msg(self.widget_id) {
            match msg {
                Msg::Resize { height, .. } => {
                    let tab_height = ctx.gfx.tab_height();
                    ctx.ui.set_scale(self.tab_bar.widget_id, tab_height);
                    ctx.ui.set_scale(self.view.widget_id, height - tab_height);
                }
                Msg::Action(action_name!(PreviousTab)) => {
                    if !self.focus_previous_tab(ctx.ui) {
                        ctx.ui.skip(self.widget_id, msg);
                    }
                }
                Msg::Action(action_name!(NextTab)) => {
                    if !self.focus_next_tab(ctx.ui) {
                        ctx.ui.skip(self.widget_id, msg);
                    }
                }
                _ => ctx.ui.skip(self.widget_id, msg),
            }
        }

        self.tab_bar.receive_msgs(&self.view, ctx);

        for tab in self.tabs.iter_mut() {
            let data = data_list.get_mut(tab.data_id()).unwrap();
            let doc = (self.get_doc_mut)(data);

            tab.receive_msgs(doc, ctx);
        }
    }

    pub fn update(&mut self, ctx: &mut Ctx) {
        self.tab_bar.update(&mut self.tabs, &self.view, ctx.ui);
    }

    pub fn animate(&mut self, data_list: &mut SlotList<T>, ctx: &mut Ctx, dt: f32) {
        self.tab_bar
            .animate(&self.tabs, data_list, self.get_doc, &self.view, ctx, dt);

        let focused_index = self.focused_tab_index(ctx.ui);
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

    pub fn draw(&mut self, background: Option<Color>, data_list: &mut SlotList<T>, ctx: &mut Ctx) {
        self.tab_bar.draw(
            self.focused_tab_index(ctx.ui),
            ctx.ui.is_in_focused_hierarchy(self.widget_id),
            background,
            &self.tabs,
            data_list,
            self.get_doc,
            ctx,
        );

        let get_doc_mut = self.get_doc_mut;

        if let Some((tab, data)) =
            self.get_tab_with_data_mut(self.focused_tab_index(ctx.ui), data_list)
        {
            tab.draw((None, background), get_doc_mut(data), ctx);
        }
    }

    fn get_tab_with_data_mut<'a>(
        &'a mut self,
        tab_index: usize,
        data_list: &'a mut SlotList<T>,
    ) -> Option<(&'a mut Tab, &'a mut T)> {
        let tab = self.tabs.get_mut(tab_index)?;
        let data = data_list.get_mut(tab.data_id())?;

        Some((tab, data))
    }

    fn get_tab_with_data<'a>(
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
        ui: &Ui,
    ) -> Option<(&'a mut Tab, &'a mut T)> {
        self.get_tab_with_data_mut(self.focused_tab_index(ui), data_list)
    }

    pub fn get_focused_tab_with_data<'a>(
        &'a self,
        data_list: &'a SlotList<T>,
        ui: &Ui,
    ) -> Option<(&'a Tab, &'a T)> {
        self.get_tab_with_data(self.focused_tab_index(ui), data_list)
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
        let tab = Tab::new(self.view.widget_id, data_id, ctx.ui);

        data_list
            .get_mut(data_id)
            .map(|data| (self.get_doc_mut)(data))
            .expect("tried to add a tab referencing a non-existent data")
            .add_usage();

        let index = (self.focused_tab_index(ctx.ui) + 1).min(self.tabs.len());

        self.tabs.insert(index, tab);
        self.tab_bar.tab_animation_states.insert(
            index,
            TabAnimationState {
                target: Rect::ZERO,
                x: self
                    .tab_bar
                    .tab_animation_states
                    .get(index.saturating_sub(1))
                    .map(|previous| previous.x)
                    .unwrap_or_default(),
            },
        );

        self.view.set_focused_index(index, ctx.ui);
        self.view.focus(ctx.ui);
    }

    pub fn remove_tab(&mut self, data_list: &mut SlotList<T>, ui: &mut Ui) {
        let get_doc_mut = self.get_doc_mut;
        let index = self.focused_tab_index(ui);

        let Some((_, data)) = self.get_tab_with_data_mut(index, data_list) else {
            return;
        };

        (get_doc_mut)(data).remove_usage();

        if let Some(child_id) = ui.child_ids(self.view.widget_id).get(index) {
            ui.remove_widget(*child_id);
        } else {
            return;
        }

        self.tabs.remove(index);
        self.tab_bar.tab_animation_states.remove(index);

        let focused_index = index.min(self.tabs.len());
        self.view.set_focused_index(focused_index, ui);
        self.view.focus(ui);
    }

    fn focus_next_tab(&self, ui: &mut Ui) -> bool {
        let focused_index = self.focused_tab_index(ui);

        if focused_index + 1 >= self.tabs.len() {
            return false;
        }

        self.view.set_focused_index(focused_index + 1, ui);
        self.view.focus(ui);

        true
    }

    fn focus_previous_tab(&self, ui: &mut Ui) -> bool {
        let focused_index = self.focused_tab_index(ui);

        if focused_index == 0 {
            return false;
        }

        self.view.set_focused_index(focused_index - 1, ui);
        self.view.focus(ui);

        true
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

    // TODO: This shouldn't be an Option since panes should always have > 0 tabs. Or the tab bar code that assumes > 0 tabs needs to be updated.
    pub fn get_focused_tab(&self, ui: &Ui) -> Option<&Tab> {
        let focused_index = self.focused_tab_index(ui);
        self.tabs.get(focused_index)
    }

    pub fn set_focused_tab_index(&self, index: usize, ui: &mut Ui) {
        self.view.set_focused_index(index, ui);
    }

    pub fn focused_tab_index(&self, ui: &Ui) -> usize {
        self.view.focused_index(ui)
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }
}
