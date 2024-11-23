use crate::{
    config::Config,
    geometry::{rect::Rect, side::SIDE_ALL, visual_position::VisualPosition},
    input::{
        key::Key,
        keybind::{Keybind, MOD_ALT, MOD_CTRL, MOD_CTRL_ALT},
        mouse_button::MouseButton,
        mousebind::Mousebind,
    },
    platform::{gfx::Gfx, window::Window},
    temp_buffer::TempBuffer,
    text::{
        cursor_index::CursorIndex,
        line_pool::{Line, LinePool},
        selection::Selection,
    },
    ui::command_palette::CommandPalette,
};

use super::{doc_list::DocList, pane::Pane};

pub struct Editor {
    doc_list: DocList,
    // There should always be at least one pane.
    panes: Vec<Pane>,
    focused_pane_index: usize,

    bounds: Rect,
    completion_result_bounds: Rect,
    completion_results_bounds: Rect,

    completion_results: Vec<Line>,
    completion_result_pool: LinePool,
    completion_prefix_len: usize,
}

impl Editor {
    pub fn new(config: &Config, line_pool: &mut LinePool, time: f32) -> Self {
        let mut editor = Self {
            doc_list: DocList::new(),
            panes: Vec::new(),
            focused_pane_index: 0,

            bounds: Rect::zero(),
            completion_result_bounds: Rect::zero(),
            completion_results_bounds: Rect::zero(),

            completion_results: Vec::new(),
            completion_result_pool: LinePool::new(),
            completion_prefix_len: 0,
        };

        editor
            .panes
            .push(Pane::new(&mut editor.doc_list, config, line_pool, time));

        editor
    }

    pub fn is_animating(&self) -> bool {
        for pane in &self.panes {
            if pane.is_animating() {
                return true;
            }
        }

        false
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx) {
        self.bounds = bounds;

        let mut pane_bounds = Rect::new(
            self.bounds.x,
            self.bounds.y,
            (self.bounds.width / self.panes.len() as f32).ceil(),
            self.bounds.height,
        );

        for pane in &mut self.panes {
            pane.layout(pane_bounds, gfx, &mut self.doc_list);
            pane_bounds.x += pane_bounds.width;
        }

        self.completion_result_bounds =
            Rect::new(0.0, 0.0, gfx.glyph_width() * 20.0, gfx.line_height() * 1.25);

        self.completion_results_bounds = Rect::zero();

        if self.completion_results.is_empty() {
            return;
        }

        let focused_pane = &self.panes[self.focused_pane_index];

        let Some((tab, doc)) =
            focused_pane.get_tab_with_doc(focused_pane.focused_tab_index(), &self.doc_list)
        else {
            return;
        };

        let cursor_position = doc.get_cursor(CursorIndex::Main).position;
        let cursor_visual_position = doc
            .position_to_visual(cursor_position, tab.camera.x(), tab.camera.y(), gfx)
            .offset_by(tab.doc_bounds());

        self.completion_results_bounds = Rect::new(
            cursor_visual_position.x - self.completion_prefix_len as f32 * gfx.glyph_width(),
            cursor_visual_position.y + gfx.line_height(),
            self.completion_result_bounds.width,
            self.completion_result_bounds.height * self.completion_results.len() as f32,
        )
        .add_margin(gfx.border_width());
    }

    pub fn update(
        &mut self,
        command_palette: &mut CommandPalette,
        window: &mut Window,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
        dt: f32,
    ) {
        let mut mousebind_handler = window.get_mousebind_handler();

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
                    if let Some((i, _)) = self
                        .panes
                        .iter()
                        .enumerate()
                        .filter(|(_, pane)| pane.bounds().contains_position(visual_position))
                        .nth(0)
                    {
                        self.focused_pane_index = i;
                    }

                    mousebind_handler.unprocessed(window, mousebind);
                }
                _ => mousebind_handler.unprocessed(window, mousebind),
            }
        }

        let mut keybind_handler = window.get_keybind_handler();

        while let Some(keybind) = keybind_handler.next(window) {
            match keybind {
                Keybind {
                    key: Key::N,
                    mods: MOD_CTRL_ALT,
                } => {
                    self.add_pane(config, line_pool, time);
                }
                Keybind {
                    key: Key::W,
                    mods: MOD_CTRL_ALT,
                } => {
                    self.close_pane(config, line_pool, time);
                }
                Keybind {
                    key: Key::PageUp,
                    mods: MOD_CTRL | MOD_CTRL_ALT,
                } => {
                    let pane = &self.panes[self.focused_pane_index];

                    if (keybind.mods & MOD_ALT != 0 || pane.focused_tab_index() == 0)
                        && self.focused_pane_index > 0
                    {
                        self.focused_pane_index -= 1;
                    } else {
                        keybind_handler.unprocessed(window, keybind);
                    }
                }
                Keybind {
                    key: Key::PageDown,
                    mods: MOD_CTRL | MOD_CTRL_ALT,
                } => {
                    let pane = &self.panes[self.focused_pane_index];

                    if (keybind.mods & MOD_ALT != 0
                        || pane.focused_tab_index() == pane.tabs_len() - 1)
                        && self.focused_pane_index < self.panes.len() - 1
                    {
                        self.focused_pane_index += 1;
                    } else {
                        keybind_handler.unprocessed(window, keybind);
                    }
                }
                _ => keybind_handler.unprocessed(window, keybind),
            }
        }

        let pane = &mut self.panes[self.focused_pane_index];

        pane.update(
            &mut self.doc_list,
            command_palette,
            window,
            line_pool,
            text_buffer,
            config,
            time,
        );

        if let Some((tab, doc)) = pane.get_tab_with_doc(pane.focused_tab_index(), &self.doc_list) {
            for result in self.completion_results.drain(..) {
                self.completion_result_pool.push(result);
            }

            self.completion_prefix_len = 0;

            let position = doc.get_cursor(CursorIndex::Main).position;
            let word_selection = doc.select_current_word_at_position(position);
            let word_selection = Selection {
                start: word_selection.start,
                end: word_selection.end.min(position),
            };

            if let Some(prefix) = doc
                .get_line(position.y)
                .filter(|_| word_selection.start.y == word_selection.end.y)
                .map(|line| &line[word_selection.start.x as usize..word_selection.end.x as usize])
                .filter(|prefix| !prefix.is_empty())
            {
                self.completion_prefix_len = prefix.len();

                doc.tokens().traverse(
                    prefix,
                    &mut self.completion_results,
                    &mut self.completion_result_pool,
                );
            }
        }

        if pane.tabs_len() == 0 {
            self.close_pane(config, line_pool, time);
        }

        for pane in &mut self.panes {
            pane.update_camera(&mut self.doc_list, window, dt);
        }

        window.clear_inputs();
    }

    pub fn draw(&mut self, config: &Config, gfx: &mut Gfx, is_focused: bool) {
        for (i, pane) in self.panes.iter_mut().enumerate() {
            let is_focused = is_focused && i == self.focused_pane_index;

            pane.draw(&mut self.doc_list, config, gfx, is_focused);
        }

        gfx.begin(Some(self.bounds));

        gfx.add_bordered_rect(
            self.completion_results_bounds,
            SIDE_ALL,
            &config.theme.background,
            &config.theme.border,
        );

        for (i, result) in self.completion_results.iter().enumerate() {
            let result_bounds = self
                .completion_result_bounds
                .offset_by(self.completion_results_bounds)
                .shift_y(i as f32 * self.completion_result_bounds.height);

            gfx.add_text(
                result.iter().copied(),
                result_bounds.x + gfx.glyph_width() / 2.0,
                result_bounds.y + (result_bounds.height - gfx.glyph_height()) / 2.0,
                &config.theme.normal,
            );
        }

        gfx.end();
    }

    fn clamp_focused_pane(&mut self) {
        if self.focused_pane_index >= self.panes.len() {
            if self.panes.is_empty() {
                self.focused_pane_index = 0;
            } else {
                self.focused_pane_index = self.panes.len() - 1;
            }
        }
    }

    fn add_pane(&mut self, config: &Config, line_pool: &mut LinePool, time: f32) {
        let pane = Pane::new(&mut self.doc_list, config, line_pool, time);

        if self.focused_pane_index >= self.panes.len() {
            self.panes.push(pane);
        } else {
            self.panes.insert(self.focused_pane_index + 1, pane);
            self.focused_pane_index += 1;
        }
    }

    fn close_pane(&mut self, config: &Config, line_pool: &mut LinePool, time: f32) {
        if self.panes.len() == 1 {
            return;
        }

        if !self.panes[self.focused_pane_index].close_all_tabs(
            &mut self.doc_list,
            config,
            line_pool,
            time,
        ) {
            return;
        }

        self.panes.remove(self.focused_pane_index);
        self.clamp_focused_pane();
    }

    pub fn on_close(&mut self, config: &Config, line_pool: &mut LinePool, time: f32) {
        self.doc_list
            .confirm_close_all("exiting", config, line_pool, time);
    }

    pub fn get_focused_pane_and_doc_list(&mut self) -> (&mut Pane, &mut DocList) {
        (&mut self.panes[self.focused_pane_index], &mut self.doc_list)
    }
}
