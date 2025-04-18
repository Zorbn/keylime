use crate::{
    ctx::Ctx,
    geometry::{rect::Rect, visual_position::VisualPosition},
    input::{action::action_name, mouse_button::MouseButton, mousebind::Mousebind},
    platform::{gfx::Gfx, window::Window},
    text::doc::Doc,
};

use super::{
    color::Color,
    core::{Ui, Widget},
    slot_list::SlotList,
    tab::Tab,
};

pub struct Pane<T> {
    pub tabs: Vec<Tab>,
    pub focused_tab_index: usize,
    bounds: Rect,

    get_doc: fn(&T) -> &Doc,
    get_doc_mut: fn(&mut T) -> &mut Doc,
}

impl<T> Pane<T> {
    pub fn new(get_doc: fn(&T) -> &Doc, get_doc_mut: fn(&mut T) -> &mut Doc) -> Self {
        Self {
            tabs: Vec::new(),
            focused_tab_index: 0,
            bounds: Rect::ZERO,

            get_doc,
            get_doc_mut,
        }
    }

    pub fn is_animating(&self) -> bool {
        self.tabs
            .get(self.focused_tab_index)
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

    pub fn update(&mut self, widget: &mut Widget, ui: &mut Ui, window: &mut Window) {
        let mut mousebind_handler = ui.get_mousebind_handler(widget, window);

        while let Some(mousebind) = mousebind_handler.next(window) {
            let visual_position =
                VisualPosition::new(mousebind.x - self.bounds.x, mousebind.y - self.bounds.y);

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: 0,
                    is_drag: false,
                    ..
                } => {
                    match self
                        .tabs
                        .iter()
                        .enumerate()
                        .filter(|(_, tab)| {
                            tab.tab_bounds()
                                .unoffset_by(self.bounds)
                                .contains_position(visual_position)
                        })
                        .nth(0)
                    {
                        Some((i, _)) => self.focused_tab_index = i,
                        _ => mousebind_handler.unprocessed(window, mousebind),
                    }
                }
                _ => mousebind_handler.unprocessed(window, mousebind),
            }
        }

        let mut action_handler = ui.get_action_handler(widget, window);

        while let Some(action) = action_handler.next(window) {
            match action {
                action_name!(PreviousTab) => {
                    if self.focused_tab_index > 0 {
                        self.focused_tab_index -= 1;
                    }
                }
                action_name!(NextTab) => {
                    if self.focused_tab_index < self.tabs.len() - 1 {
                        self.focused_tab_index += 1;
                    }
                }
                _ => action_handler.unprocessed(window, action),
            }
        }
    }

    pub fn update_camera(
        &mut self,
        widget: &mut Widget,
        ui: &mut Ui,
        data_list: &mut SlotList<T>,
        ctx: &mut Ctx,
        dt: f32,
    ) {
        let get_doc = self.get_doc;

        if let Some((tab, data)) = self.get_tab_with_data_mut(self.focused_tab_index, data_list) {
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

        for i in 0..self.tabs.len() {
            let get_doc = self.get_doc;

            let Some((tab, data)) = self.get_tab_with_data(i, data_list) else {
                continue;
            };

            Self::draw_tab(tab, get_doc(data), self.bounds, ctx);
        }

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        let focused_tab_bounds = if let Some(tab) = self.tabs.get(self.focused_tab_index) {
            let tab_bounds = tab.tab_bounds().unoffset_by(self.bounds);

            if is_focused {
                gfx.add_rect(tab_bounds.top_border(gfx.border_width()), theme.keyword);
            }

            tab_bounds
        } else {
            Rect::ZERO
        };

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

        if let Some((tab, data)) = self.get_tab_with_data_mut(self.focused_tab_index, data_list) {
            tab.draw(default_background, get_doc_mut(data), ctx, is_focused);
        }
    }

    fn draw_tab(tab: &Tab, doc: &Doc, bounds: Rect, ctx: &mut Ctx) {
        let tab_bounds = tab.tab_bounds().unoffset_by(bounds);
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        gfx.add_rect(tab_bounds.left_border(gfx.border_width()), theme.border);

        let text_x = (tab_bounds.x + gfx.glyph_width() * 2.0).floor();
        let text_y = gfx.border_width() + gfx.tab_padding_y();
        let text_width = gfx.add_text(doc.file_name(), text_x, text_y, theme.normal);

        if !doc.is_saved() {
            gfx.add_text("*", text_x + text_width, text_y, theme.symbol);
        }

        gfx.add_rect(tab_bounds.right_border(gfx.border_width()), theme.border);
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

    fn clamp_focused_tab(&mut self) {
        if self.focused_tab_index >= self.tabs.len() {
            if self.tabs.is_empty() {
                self.focused_tab_index = 0;
            } else {
                self.focused_tab_index = self.tabs.len() - 1;
            }
        }
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

        if self.focused_tab_index >= self.tabs.len() {
            self.tabs.push(tab);
        } else {
            self.tabs.insert(self.focused_tab_index + 1, tab);
            self.focused_tab_index += 1;
        }
    }

    pub fn remove_tab(&mut self, data_list: &mut SlotList<T>) {
        let get_doc_mut = self.get_doc_mut;

        let Some((_, data)) = self.get_tab_with_data_mut(self.focused_tab_index, data_list) else {
            return;
        };

        (get_doc_mut)(data).remove_usage();

        self.tabs.remove(self.focused_tab_index);
        self.clamp_focused_tab();
    }

    pub fn tabs_len(&self) -> usize {
        self.tabs.len()
    }

    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn focused_tab_index(&self) -> usize {
        self.focused_tab_index
    }
}
