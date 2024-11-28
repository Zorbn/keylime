use crate::{
    config::{theme::Theme, Config},
    geometry::{rect::Rect, visual_position::VisualPosition},
    input::{
        key::Key,
        keybind::{Keybind, MOD_CTRL},
        mouse_button::MouseButton,
        mousebind::Mousebind,
    },
    platform::gfx::Gfx,
    text::doc::Doc,
};

use super::{color::Color, doc_list::DocList, tab::Tab, widget::Widget, UiHandle};

pub struct Pane {
    pub tabs: Vec<Tab>,
    pub focused_tab_index: usize,
    bounds: Rect,
}

impl Pane {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            focused_tab_index: 0,
            bounds: Rect::zero(),
        }
    }

    pub fn is_animating(&self) -> bool {
        if let Some(tab) = self.tabs.get(self.focused_tab_index) {
            tab.is_animating()
        } else {
            false
        }
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx, doc_list: &mut DocList) {
        self.bounds = bounds;

        let mut tab_x = bounds.x;
        let tab_height = gfx.tab_height();

        for i in 0..self.tabs.len() {
            let Some((tab, doc)) = self.get_tab_with_doc_mut(i, doc_list) else {
                return;
            };

            let tab_width = gfx.glyph_width() * 4.0
                + Gfx::measure_text(doc.file_name().chars()) as f32 * gfx.glyph_width();

            let tab_bounds = Rect::new(tab_x, bounds.y, tab_width, tab_height);
            let doc_bounds = bounds.shrink_top_by(tab_bounds);

            tab_x += tab_width - gfx.border_width();

            tab.layout(tab_bounds, doc_bounds, doc, gfx);
        }
    }

    pub fn update(&mut self, widget: &Widget, ui: &mut UiHandle) {
        let mut mousebind_handler = widget.get_mousebind_handler(ui);

        while let Some(mousebind) = mousebind_handler.next(ui.window) {
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
                        _ => mousebind_handler.unprocessed(ui.window, mousebind),
                    }
                }
                _ => mousebind_handler.unprocessed(ui.window, mousebind),
            }
        }

        let mut keybind_handler = widget.get_keybind_handler(ui);

        while let Some(keybind) = keybind_handler.next(ui.window) {
            match keybind {
                Keybind {
                    key: Key::PageUp,
                    mods: MOD_CTRL,
                } => {
                    if self.focused_tab_index > 0 {
                        self.focused_tab_index -= 1;
                    }
                }
                Keybind {
                    key: Key::PageDown,
                    mods: MOD_CTRL,
                } => {
                    if self.focused_tab_index < self.tabs.len() - 1 {
                        self.focused_tab_index += 1;
                    }
                }
                _ => keybind_handler.unprocessed(ui.window, keybind),
            }
        }
    }

    pub fn update_camera(&mut self, ui: &mut UiHandle, doc_list: &mut DocList, dt: f32) {
        if let Some((tab, doc)) = self.get_tab_with_doc_mut(self.focused_tab_index, doc_list) {
            tab.update_camera(ui, doc, dt);
        }
    }

    pub fn draw(
        &mut self,
        default_background: Option<Color>,
        doc_list: &mut DocList,
        config: &Config,
        gfx: &mut Gfx,
        is_focused: bool,
    ) {
        let tab_height = gfx.tab_height();

        gfx.begin(Some(self.bounds));

        gfx.add_rect(
            self.bounds
                .left_border(gfx.border_width())
                .unoffset_by(self.bounds),
            config.theme.border,
        );

        gfx.add_rect(
            self.bounds
                .top_border(gfx.border_width())
                .unoffset_by(self.bounds),
            config.theme.border,
        );

        for i in 0..self.tabs.len() {
            let Some((tab, doc)) = self.get_tab_with_doc(i, doc_list) else {
                continue;
            };

            Self::draw_tab(tab, doc, self.bounds, &config.theme, gfx);
        }

        let focused_tab_bounds = if let Some(tab) = is_focused
            .then(|| self.tabs.get(self.focused_tab_index))
            .flatten()
        {
            let tab_bounds = tab.tab_bounds().unoffset_by(self.bounds);

            gfx.add_rect(
                tab_bounds.top_border(gfx.border_width()),
                config.theme.keyword,
            );

            tab_bounds
        } else {
            Rect::zero()
        };

        gfx.add_rect(
            Rect::from_sides(
                0.0,
                tab_height - gfx.border_width(),
                focused_tab_bounds.x,
                tab_height,
            ),
            config.theme.border,
        );

        gfx.add_rect(
            Rect::from_sides(
                focused_tab_bounds.x + focused_tab_bounds.width,
                tab_height - gfx.border_width(),
                self.bounds.width,
                tab_height,
            ),
            config.theme.border,
        );

        gfx.end();

        if let Some((tab, doc)) = self.get_tab_with_doc_mut(self.focused_tab_index, doc_list) {
            tab.draw(default_background, doc, config, gfx, is_focused);
        }
    }

    fn draw_tab(tab: &Tab, doc: &Doc, bounds: Rect, theme: &Theme, gfx: &mut Gfx) {
        let tab_padding_y = gfx.tab_padding_y();
        let tab_bounds = tab.tab_bounds().unoffset_by(bounds);

        gfx.add_rect(tab_bounds.left_border(gfx.border_width()), theme.border);

        let text_x = (tab_bounds.x + gfx.glyph_width() * 2.0).floor();

        let text_width = gfx.add_text(doc.file_name().chars(), text_x, tab_padding_y, theme.normal);

        if !doc.is_saved() {
            gfx.add_text(
                "*".chars(),
                text_x + text_width as f32 * gfx.glyph_width(),
                tab_padding_y,
                theme.symbol,
            );
        }

        gfx.add_rect(tab_bounds.right_border(gfx.border_width()), theme.border);
    }

    pub fn get_tab_with_doc_mut<'a>(
        &'a mut self,
        tab_index: usize,
        doc_list: &'a mut DocList,
    ) -> Option<(&'a mut Tab, &'a mut Doc)> {
        if let Some(tab) = self.tabs.get_mut(tab_index) {
            if let Some(doc) = doc_list.get_mut(tab.doc_index()) {
                return Some((tab, doc));
            }
        }

        None
    }

    pub fn get_tab_with_doc<'a>(
        &'a self,
        tab_index: usize,
        doc_list: &'a DocList,
    ) -> Option<(&'a Tab, &'a Doc)> {
        if let Some(tab) = self.tabs.get(tab_index) {
            if let Some(doc) = doc_list.get(tab.doc_index()) {
                return Some((tab, doc));
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

    pub fn get_existing_tab_for_doc(&self, doc_index: usize) -> Option<usize> {
        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.doc_index() == doc_index {
                return Some(i);
            }
        }

        None
    }

    pub fn add_tab(&mut self, doc_index: usize, doc_list: &mut DocList) {
        let tab = Tab::new(doc_index);

        doc_list
            .get_mut(doc_index)
            .expect("tried to add a tab referencing a non-existent doc")
            .add_usage();

        if self.focused_tab_index >= self.tabs.len() {
            self.tabs.push(tab);
        } else {
            self.tabs.insert(self.focused_tab_index + 1, tab);
            self.focused_tab_index += 1;
        }
    }

    pub fn remove_tab(&mut self, doc_list: &mut DocList) {
        let Some((_, doc)) = self.get_tab_with_doc_mut(self.focused_tab_index, doc_list) else {
            return;
        };

        doc.remove_usage();

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
