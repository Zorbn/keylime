use crate::{
    config::Config,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
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
        doc::Doc,
        line_pool::{Line, LinePool},
    },
};

use super::{
    command_palette::CommandPalette,
    doc_list::DocList,
    pane::Pane,
    result_list::{ResultList, ResultListInput},
};

const MAX_VISIBLE_COMPLETION_RESULTS: usize = 10;

pub struct Editor {
    doc_list: DocList,
    // There should always be at least one pane.
    panes: Vec<Pane>,
    focused_pane_index: usize,

    bounds: Rect,

    completion_result_list: ResultList<Line>,
    completion_result_pool: LinePool,
    completion_prefix: Vec<char>,
}

impl Editor {
    pub fn new(config: &Config, line_pool: &mut LinePool, time: f32) -> Self {
        let mut editor = Self {
            doc_list: DocList::new(),
            panes: Vec::new(),
            focused_pane_index: 0,

            bounds: Rect::zero(),

            completion_result_list: ResultList::new(MAX_VISIBLE_COMPLETION_RESULTS),
            completion_result_pool: LinePool::new(),
            completion_prefix: Vec::new(),
        };

        editor
            .panes
            .push(Pane::new(&mut editor.doc_list, config, line_pool, time));

        editor
    }

    pub fn is_animating(&self) -> bool {
        if self.completion_result_list.is_animating() {
            return true;
        }

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

        let focused_pane = &self.panes[self.focused_pane_index];

        let Some((tab, doc)) =
            focused_pane.get_tab_with_doc(focused_pane.focused_tab_index(), &self.doc_list)
        else {
            return;
        };

        let cursor_position = doc.get_cursor(CursorIndex::Main).position;
        let cursor_visual_position = doc
            .position_to_visual(cursor_position, tab.camera.position(), gfx)
            .offset_by(tab.doc_bounds());

        let min_y = self.completion_result_list.min_visible_result_index();
        let max_y =
            (min_y + MAX_VISIBLE_COMPLETION_RESULTS).min(self.completion_result_list.results.len());
        let mut longest_visible_result = 0;

        for y in min_y..max_y {
            longest_visible_result =
                longest_visible_result.max(self.completion_result_list.results[y].len());
        }

        self.completion_result_list.layout(
            Rect::new(
                cursor_visual_position.x
                    - (self.completion_prefix.len() as f32 + 1.0) * gfx.glyph_width()
                    + gfx.border_width(),
                cursor_visual_position.y + gfx.line_height(),
                (longest_visible_result as f32 + 2.0) * gfx.glyph_width(),
                0.0,
            ),
            gfx,
        );
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

        let are_results_visible = self.is_cursor_visible(window.gfx());
        let are_results_focused = !self.completion_result_list.results.is_empty();

        let result_input = self.completion_result_list.update(
            window,
            are_results_visible,
            are_results_focused,
            dt,
        );

        match result_input {
            ResultListInput::None => {}
            ResultListInput::Complete | ResultListInput::Submit { .. } => {
                if let Some(result) = self.completion_result_list.get_selected_result() {
                    let pane = &mut self.panes[self.focused_pane_index];

                    if let Some((_, doc)) =
                        pane.get_tab_with_doc_mut(pane.focused_tab_index(), &mut self.doc_list)
                    {
                        doc.insert_at_cursors(
                            &result[self.completion_prefix.len()..],
                            line_pool,
                            time,
                        );
                    }
                }

                Self::clear_completions(
                    &mut self.completion_result_list,
                    &mut self.completion_result_pool,
                );
            }
            ResultListInput::Close => {
                Self::clear_completions(
                    &mut self.completion_result_list,
                    &mut self.completion_result_pool,
                );
            }
        }

        let handled_doc_info = self.get_doc_info();
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

        if pane.tabs_len() == 0 {
            self.close_pane(config, line_pool, time);
        }

        for pane in &mut self.panes {
            pane.update_camera(&mut self.doc_list, window, dt);
        }

        self.update_completions(handled_doc_info);

        window.clear_inputs();
    }

    pub fn draw(&mut self, config: &Config, gfx: &mut Gfx, is_focused: bool) {
        for (i, pane) in self.panes.iter_mut().enumerate() {
            let is_focused = is_focused && i == self.focused_pane_index;

            pane.draw(&mut self.doc_list, config, gfx, is_focused);
        }

        if self.is_cursor_visible(gfx) {
            self.completion_result_list
                .draw(config, gfx, |result| result.iter().copied());
        }
    }

    fn get_completion_prefix(doc: &Doc) -> Option<&[char]> {
        let prefix_end = doc.get_cursor(CursorIndex::Main).position;

        if prefix_end.x == 0 {
            return Some(&[]);
        }

        let mut prefix_start = prefix_end;

        while prefix_start.x > 0 {
            let next_start = doc.move_position(prefix_start, Position::new(-1, 0));

            let c = doc.get_char(next_start);

            if !c.is_alphanumeric() && c != '_' {
                break;
            }

            prefix_start = next_start;
        }

        doc.get_line(prefix_end.y)
            .map(|line| &line[prefix_start.x as usize..prefix_end.x as usize])
    }

    fn clear_completions(
        completion_result_list: &mut ResultList<Line>,
        completion_result_pool: &mut LinePool,
    ) {
        for result in completion_result_list.drain() {
            completion_result_pool.push(result);
        }
    }

    fn update_completions(
        &mut self,
        (handled_version, handled_position): (Option<usize>, Option<Position>),
    ) {
        let (version, position) = self.get_doc_info();

        let is_version_different = version != handled_version;
        let is_position_different = position != handled_position;

        if is_version_different || is_position_different {
            self.completion_prefix.clear();

            Self::clear_completions(
                &mut self.completion_result_list,
                &mut self.completion_result_pool,
            );
        }

        if !is_version_different {
            return;
        }

        let pane = &mut self.panes[self.focused_pane_index];

        let Some((_, doc)) = pane.get_tab_with_doc(pane.focused_tab_index(), &self.doc_list) else {
            return;
        };

        let Some(prefix) =
            Self::get_completion_prefix(doc).filter(|prefix| self.completion_prefix != *prefix)
        else {
            return;
        };

        self.completion_prefix.extend_from_slice(prefix);

        if !prefix.is_empty() {
            doc.tokens().traverse(
                prefix,
                &mut self.completion_result_list.results,
                &mut self.completion_result_pool,
            );
        }
    }

    fn get_doc_info(&self) -> (Option<usize>, Option<Position>) {
        let pane = &self.panes[self.focused_pane_index];

        pane.get_tab_with_doc(pane.focused_tab_index(), &self.doc_list)
            .map(|(_, doc)| (doc.version(), doc.get_cursor(CursorIndex::Main).position))
            .map(|info| (Some(info.0), Some(info.1)))
            .unwrap_or_default()
    }

    fn is_cursor_visible(&self, gfx: &Gfx) -> bool {
        let pane = &self.panes[self.focused_pane_index];

        let Some((tab, doc)) = pane.get_tab_with_doc(pane.focused_tab_index(), &self.doc_list)
        else {
            return false;
        };

        let cursor_position = doc.get_cursor(CursorIndex::Main).position;
        let cursor_visual_position = doc
            .position_to_visual(cursor_position, tab.camera.position(), gfx)
            .shift_y(gfx.line_height())
            .offset_by(tab.doc_bounds());

        tab.doc_bounds().contains_position(cursor_visual_position)
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
