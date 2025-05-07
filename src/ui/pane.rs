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
        mousebind::{MouseClickKind, Mousebind},
    },
    platform::{gfx::Gfx, window::Window},
    text::doc::Doc,
};

use super::{
    color::Color,
    core::{Ui, Widget},
    focus_list::FocusList,
    slot_list::SlotList,
    tab::Tab,
};

pub struct Pane<T> {
    pub tabs: FocusList<Tab>,
    bounds: Rect,
    dragged_tab_offset: Option<f32>,

    get_doc: fn(&T) -> &Doc,
    get_doc_mut: fn(&mut T) -> &mut Doc,
}

impl<T> Pane<T> {
    pub fn new(get_doc: fn(&T) -> &Doc, get_doc_mut: fn(&mut T) -> &mut Doc) -> Self {
        Self {
            tabs: FocusList::new(),
            bounds: Rect::ZERO,
            dragged_tab_offset: None,

            get_doc,
            get_doc_mut,
        }
    }

    pub fn is_animating(&self) -> bool {
        self.tabs
            .get_focused()
            .is_some_and(|tab| tab.is_animating())
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &mut Gfx, data_list: &mut SlotList<T>) {
        self.bounds = bounds;

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
            let doc_bounds = bounds.shrink_top_by(tab_bounds);

            tab_x += tab_width - gfx.border_width();

            tab.layout(tab_bounds, doc_bounds, doc, gfx);
        }
    }

    pub fn update(&mut self, widget: &Widget, ui: &mut Ui, window: &mut Window) {
        let mut mousebind_handler = ui.get_mousebind_handler(widget, window);

        while let Some(mousebind) = mousebind_handler.next(window) {
            let visual_position = VisualPosition::new(mousebind.x, mousebind.y);

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: Mods::NONE,
                    kind: MouseClickKind::Press,
                    ..
                } => {
                    let mut offset = 0.0;

                    let index = self.tabs.iter().position(|tab| {
                        let tab_bounds = tab.tab_bounds();

                        offset = tab_bounds.x - visual_position.x - self.bounds.x;

                        tab_bounds.contains_position(visual_position)
                    });

                    if let Some(index) = index {
                        self.tabs.set_focused_index(index);
                        self.dragged_tab_offset = Some(offset);
                    } else {
                        mousebind_handler.unprocessed(window, mousebind);
                    }
                }
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: Mods::NONE,
                    kind: MouseClickKind::Release,
                    ..
                } => self.dragged_tab_offset = None,
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: Mods::NONE,
                    kind: MouseClickKind::Drag,
                    ..
                } if self.dragged_tab_offset.is_some() => {
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
                _ => mousebind_handler.unprocessed(window, mousebind),
            }
        }

        let mut action_handler = ui.get_action_handler(widget, window);

        while let Some(action) = action_handler.next(window) {
            match action {
                action_name!(PreviousTab) => self.tabs.focus_previous(),
                action_name!(NextTab) => self.tabs.focus_next(),
                _ => action_handler.unprocessed(window, action),
            }
        }
    }

    pub fn update_camera(
        &mut self,
        widget: &Widget,
        ui: &mut Ui,
        data_list: &mut SlotList<T>,
        ctx: &mut Ctx,
        dt: f32,
    ) {
        let get_doc = self.get_doc;

        if let Some((tab, data)) = self.get_tab_with_data_mut(self.tabs.focused_index(), data_list)
        {
            tab.update_camera(widget, ui, get_doc(data), ctx, dt);
        }
    }

    pub fn draw(
        &mut self,
        default_background: Option<Color>,
        data_list: &mut SlotList<T>,
        ctx: &mut Ctx,
        is_focused: bool,
    ) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;
        let tab_height = gfx.tab_height();

        gfx.begin(Some(self.bounds));

        gfx.add_rect(
            self.bounds
                .left_border(gfx.border_width())
                .unoffset_by(self.bounds),
            theme.border,
        );

        gfx.add_rect(
            self.bounds
                .top_border(gfx.border_width())
                .unoffset_by(self.bounds),
            theme.border,
        );

        if self.tabs.is_empty() {
            gfx.add_rect(
                Rect::from_sides(
                    0.0,
                    tab_height - gfx.border_width(),
                    self.bounds.width,
                    tab_height,
                ),
                theme.border,
            );

            gfx.end();

            return;
        }

        for i in 0..self.tabs.len() {
            if i == self.tabs.focused_index() {
                continue;
            }

            self.draw_tab_from_index(i, data_list, ctx);
        }

        let focused_tab_bounds =
            self.draw_tab_from_index(self.tabs.focused_index(), data_list, ctx);

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

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
                self.bounds.width,
                tab_height,
            ),
            theme.border,
        );

        gfx.end();

        let get_doc_mut = self.get_doc_mut;

        if let Some((tab, data)) = self.get_tab_with_data_mut(self.tabs.focused_index(), data_list)
        {
            tab.draw(default_background, get_doc_mut(data), ctx, is_focused);
        }
    }

    fn draw_tab_from_index(
        &mut self,
        index: usize,
        data_list: &mut SlotList<T>,
        ctx: &mut Ctx,
    ) -> Rect {
        let get_doc = self.get_doc;

        let Some((tab, data)) = self.get_tab_with_data(index, data_list) else {
            return Rect::ZERO;
        };

        Self::draw_tab(
            index == self.tabs.focused_index(),
            self.dragged_tab_offset,
            tab,
            get_doc(data),
            self.bounds,
            ctx,
        )
    }

    fn draw_tab(
        is_focused: bool,
        dragged_tab_offset: Option<f32>,
        tab: &Tab,
        doc: &Doc,
        bounds: Rect,
        ctx: &mut Ctx,
    ) -> Rect {
        let theme = &ctx.config.theme;

        let text_color = Self::get_tab_color(doc, theme, ctx);
        let mut tab_bounds = tab.tab_bounds().unoffset_by(bounds);

        if let Some(offset) = dragged_tab_offset.filter(|_| is_focused) {
            tab_bounds.x += ctx.window.get_mouse_position().x - tab_bounds.x + offset;
        };

        let gfx = &mut ctx.gfx;

        gfx.add_bordered_rect(
            tab_bounds,
            Sides::ALL.without(Side::Bottom),
            theme.background,
            theme.border,
        );

        let text_x = (tab_bounds.x + gfx.glyph_width() * 2.0).floor();
        let text_y = gfx.border_width() + gfx.tab_padding_y();
        let text_width = gfx.add_text(doc.file_name(), text_x, text_y, text_color);

        if !doc.is_saved() {
            gfx.add_text("*", text_x + text_width, text_y, theme.symbol);
        }

        if is_focused {
            gfx.add_rect(tab_bounds.top_border(gfx.border_width()), theme.keyword);
        }

        tab_bounds
    }

    fn get_tab_color(doc: &Doc, theme: &Theme, ctx: &mut Ctx) -> Color {
        for language_server in ctx.lsp.iter_servers_mut() {
            for diagnostic in language_server.get_diagnostics_mut(doc) {
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
        if let Some(tab) = self.tabs.get_mut(tab_index) {
            if let Some(data) = data_list.get_mut(tab.data_index()) {
                return Some((tab, data));
            }
        }

        None
    }

    pub fn get_tab_with_data<'a>(
        &'a self,
        tab_index: usize,
        data_list: &'a SlotList<T>,
    ) -> Option<(&'a Tab, &'a T)> {
        if let Some(tab) = self.tabs.get(tab_index) {
            if let Some(data) = data_list.get(tab.data_index()) {
                return Some((tab, data));
            }
        }

        None
    }

    pub fn get_existing_tab_for_data(&self, data_index: usize) -> Option<usize> {
        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.data_index() == data_index {
                return Some(i);
            }
        }

        None
    }

    pub fn add_tab(&mut self, data_index: usize, data_list: &mut SlotList<T>) {
        let tab = Tab::new(data_index);

        data_list
            .get_mut(data_index)
            .map(|data| (self.get_doc_mut)(data))
            .expect("tried to add a tab referencing a non-existent data")
            .add_usage();

        self.tabs.add(tab);
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

    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn set_focused_tab_index(&mut self, index: usize) {
        self.tabs.set_focused_index(index);
    }

    pub fn focused_tab_index(&self) -> usize {
        self.tabs.focused_index()
    }
}
