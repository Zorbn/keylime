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
        mousebind::{Mousebind, MousebindKind},
    },
    text::doc::Doc,
};

use super::{
    color::Color,
    core::{Ui, WidgetId, WidgetSettings},
    focus_list::FocusList,
    slot_list::{SlotId, SlotList},
    tab::Tab,
};

pub struct Pane<T> {
    pub tabs: FocusList<Tab>,
    dragged_tab_offset: Option<f32>,

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
            dragged_tab_offset: None,

            get_doc,
            get_doc_mut,

            widget_id: ui.new_widget(parent_id, WidgetSettings::default()),
        }
    }

    pub fn is_animating(&self) -> bool {
        self.tabs.get_focused().is_some_and(Tab::is_animating)
    }

    pub fn layout(&mut self, bounds: Rect, data_list: &mut SlotList<T>, ctx: &mut Ctx) {
        ctx.ui.widget_mut(self.widget_id).bounds = bounds;

        let gfx = &mut ctx.gfx;

        let mut tab_x = bounds.x;
        let tab_height = gfx.tab_height();

        for i in 0..self.tabs.len() {
            let get_doc = self.get_doc;

            let Some((tab, data)) = self.get_tab_with_data_mut(i, data_list) else {
                return;
            };

            let doc = (get_doc)(data);

            let tab_width = gfx.glyph_width() * 4.0
                + gfx.measure_text(doc.file_name()) as f32 * gfx.glyph_width();

            let tab_bounds = Rect::new(tab_x, bounds.y, tab_width, tab_height);

            let doc_bounds = bounds
                .shrink_left_by(bounds.left_border(gfx.border_width()))
                .shrink_top_by(tab_bounds);

            tab_x += tab_width - gfx.border_width();

            tab.layout(tab_bounds, doc_bounds, 0.0, doc, gfx);
        }
    }

    pub fn update(&mut self, ctx: &mut Ctx) {
        let mut global_mousebind_handler = ctx.window.mousebind_handler();

        while let Some(mousebind) = global_mousebind_handler.next(ctx.window) {
            if self.dragged_tab_offset.is_none() {
                global_mousebind_handler.unprocessed(ctx.window, mousebind);
                break;
            }

            let visual_position = VisualPosition::new(mousebind.x, mousebind.y);

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: Mods::NONE,
                    kind: MousebindKind::Release,
                    ..
                } => self.dragged_tab_offset = None,
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: Mods::NONE,
                    kind: MousebindKind::Move,
                    ..
                } => {
                    let half_focused_tab_width = self
                        .tabs
                        .get_focused()
                        .map(|tab| tab.tab_bounds().width)
                        .unwrap_or_default()
                        / 2.0;

                    let index = self.tabs.iter().enumerate().position(|(index, tab)| {
                        let tab_bounds = tab.tab_bounds();
                        let half_tab_width = tab_bounds.width / 2.0;

                        if self.tabs.focused_index() < index {
                            visual_position.x + half_focused_tab_width
                                > tab_bounds.x + half_tab_width
                                && visual_position.x < tab_bounds.right()
                        } else {
                            visual_position.x - half_focused_tab_width
                                < tab_bounds.x + half_tab_width
                                && visual_position.x > tab_bounds.x
                        }
                    });

                    let index = index.filter(|index| *index != self.tabs.focused_index());

                    if let Some(index) = index {
                        self.tabs.swap(self.tabs.focused_index(), index);
                    }
                }
                _ => global_mousebind_handler.unprocessed(ctx.window, mousebind),
            }
        }

        let mut mousebind_handler = ctx.ui.mousebind_handler(self.widget_id, ctx.window);

        while let Some(mousebind) = mousebind_handler.next(ctx.window) {
            let visual_position = VisualPosition::new(mousebind.x, mousebind.y);

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: Mods::NONE,
                    kind: MousebindKind::Press,
                    ..
                } => {
                    let mut offset = 0.0;

                    let index = self.tabs.iter().position(|tab| {
                        let bounds = ctx.ui.widget(self.widget_id).bounds;
                        let tab_bounds = tab.tab_bounds();

                        offset = tab_bounds.x - visual_position.x - bounds.x;

                        tab_bounds.contains_position(visual_position)
                    });

                    if let Some(index) = index {
                        self.tabs.set_focused_index(index);
                        self.dragged_tab_offset = Some(offset);
                    } else {
                        mousebind_handler.unprocessed(ctx.window, mousebind);
                    }
                }
                _ => mousebind_handler.unprocessed(ctx.window, mousebind),
            }
        }

        let mut action_handler = ctx.ui.action_handler(self.widget_id, ctx.window);

        while let Some(action) = action_handler.next(ctx) {
            match action {
                action_name!(PreviousTab) => self.tabs.focus_previous(),
                action_name!(NextTab) => self.tabs.focus_next(),
                _ => action_handler.unprocessed(ctx.window, action),
            }
        }
    }

    pub fn update_camera(&mut self, data_list: &mut SlotList<T>, ctx: &mut Ctx, dt: f32) {
        let widget_id = self.widget_id;
        let get_doc = self.get_doc;

        if let Some((tab, data)) = self.get_tab_with_data_mut(self.tabs.focused_index(), data_list)
        {
            tab.update_camera(widget_id, get_doc(data), ctx, dt);
        }
    }

    pub fn draw(&mut self, background: Option<Color>, data_list: &mut SlotList<T>, ctx: &mut Ctx) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;
        let bounds = ctx.ui.widget(self.widget_id).bounds;

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

        let get_doc_mut = self.get_doc_mut;
        let is_focused = ctx.ui.is_focused(self.widget_id);

        if let Some((tab, data)) = self.get_tab_with_data_mut(self.tabs.focused_index(), data_list)
        {
            tab.draw((None, background), get_doc_mut(data), is_focused, ctx);
        }
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

        let bounds = ctx.ui.widget(self.widget_id).bounds;
        let mut tab_bounds = tab.tab_bounds().unoffset_by(bounds);
        let mut tab_background = theme.background;

        if is_focused {
            if let Some(offset) = self.dragged_tab_offset {
                tab_bounds.x += ctx.window.mouse_position().x - tab_bounds.x + offset;
            }

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
        let tab = Tab::new(data_id);

        data_list
            .get_mut(data_id)
            .map(|data| (self.get_doc_mut)(data))
            .expect("tried to add a tab referencing a non-existent data")
            .add_usage();

        self.tabs.add(tab);

        let bounds = ctx.ui.widget(self.widget_id).bounds;
        self.layout(bounds, data_list, ctx);
    }

    pub fn remove_tab(&mut self, data_list: &mut SlotList<T>) {
        let get_doc_mut = self.get_doc_mut;

        let Some((_, data)) = self.get_tab_with_data_mut(self.tabs.focused_index(), data_list)
        else {
            return;
        };

        (get_doc_mut)(data).remove_usage();

        self.tabs.remove();
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    pub fn set_focused_tab_index(&mut self, index: usize) {
        self.tabs.set_focused_index(index);
    }

    pub fn focused_tab_index(&self) -> usize {
        self.tabs.focused_index()
    }
}
