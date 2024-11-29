use doc_io::confirm_close_all;
use editor_pane::EditorPane;

use crate::{
    config::Config,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    input::{
        key::Key,
        keybind::{Keybind, MOD_ALT, MOD_CTRL, MOD_CTRL_ALT},
        mouse_button::MouseButton,
        mousebind::Mousebind,
    },
    platform::gfx::Gfx,
    temp_buffer::TempBuffer,
    text::{
        cursor_index::CursorIndex,
        doc::Doc,
        line_pool::{Line, LinePool},
    },
};

use super::{
    result_list::{ResultList, ResultListInput},
    slot_list::SlotList,
    widget::Widget,
    Ui, UiHandle,
};

mod doc_io;
pub mod editor_pane;

const MAX_VISIBLE_COMPLETION_RESULTS: usize = 10;

pub struct Editor {
    doc_list: SlotList<Doc>,
    // There should always be at least one pane.
    panes: Vec<EditorPane>,
    focused_pane_index: usize,

    completion_result_list: ResultList<Line>,
    completion_result_pool: LinePool,
    completion_prefix: Vec<char>,

    pub widget: Widget,
}

impl Editor {
    pub fn new(ui: &mut Ui, line_pool: &mut LinePool) -> Self {
        let mut editor = Self {
            doc_list: SlotList::new(),
            panes: Vec::new(),
            focused_pane_index: 0,

            completion_result_list: ResultList::new(MAX_VISIBLE_COMPLETION_RESULTS),
            completion_result_pool: LinePool::new(),
            completion_prefix: Vec::new(),

            widget: Widget::new(ui, true),
        };

        editor
            .panes
            .push(EditorPane::new(&mut editor.doc_list, line_pool));

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
        let mut pane_bounds = bounds;
        pane_bounds.width = (pane_bounds.width / self.panes.len() as f32).ceil();

        for pane in &mut self.panes {
            pane.layout(pane_bounds, gfx, &mut self.doc_list);
            pane_bounds.x += pane_bounds.width;
        }

        let focused_pane = &self.panes[self.focused_pane_index];

        let Some((tab, doc)) =
            focused_pane.get_tab_with_data(focused_pane.focused_tab_index(), &self.doc_list)
        else {
            return;
        };

        let cursor_position = doc.get_cursor(CursorIndex::Main).position;
        let cursor_visual_position = doc
            .position_to_visual(cursor_position, tab.camera.position().floor(), gfx)
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

        self.widget
            .layout(&[bounds, self.completion_result_list.bounds()]);
    }

    pub fn update(
        &mut self,
        ui: &mut UiHandle,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
        dt: f32,
    ) {
        let mut char_handler = self.widget.get_char_handler(ui);

        let mut should_open_completions = char_handler
            .next(ui.window)
            .map(|c| char_handler.unprocessed(ui.window, c))
            .is_some();

        let mut mousebind_handler = self.widget.get_mousebind_handler(ui);

        while let Some(mousebind) = mousebind_handler.next(ui.window) {
            let visual_position =
                VisualPosition::new(mousebind.x, mousebind.y).unoffset_by(self.widget.bounds());

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

                    mousebind_handler.unprocessed(ui.window, mousebind);
                }
                _ => mousebind_handler.unprocessed(ui.window, mousebind),
            }
        }

        let mut keybind_handler = self.widget.get_keybind_handler(ui);

        while let Some(keybind) = keybind_handler.next(ui.window) {
            match keybind {
                Keybind {
                    key: Key::Backspace,
                    ..
                } => {
                    should_open_completions = true;

                    keybind_handler.unprocessed(ui.window, keybind);
                }
                Keybind {
                    key: Key::N,
                    mods: MOD_CTRL_ALT,
                } => {
                    self.add_pane(line_pool);
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
                        keybind_handler.unprocessed(ui.window, keybind);
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
                        keybind_handler.unprocessed(ui.window, keybind);
                    }
                }
                _ => keybind_handler.unprocessed(ui.window, keybind),
            }
        }

        let are_results_visible = self.is_cursor_visible(ui.gfx());
        let are_results_focused = !self.completion_result_list.results.is_empty();

        let result_input = self.completion_result_list.update(
            &self.widget,
            ui,
            are_results_visible,
            are_results_focused,
            dt,
        );

        match result_input {
            ResultListInput::None => {}
            ResultListInput::Complete | ResultListInput::Submit { .. } => {
                if let Some(result) = self.completion_result_list.get_selected_result() {
                    let pane = &mut self.panes[self.focused_pane_index];
                    let focused_tab_index = pane.focused_tab_index();

                    if let Some((_, doc)) =
                        pane.get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
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

        let handled_position = self.get_cursor_position();
        let pane = &mut self.panes[self.focused_pane_index];

        pane.update(
            &self.widget,
            ui,
            &mut self.doc_list,
            line_pool,
            text_buffer,
            config,
            time,
        );

        if pane.tabs_len() == 0 {
            self.close_pane(config, line_pool, time);
        }

        for pane in &mut self.panes {
            pane.update_camera(ui, &mut self.doc_list, dt);
        }

        self.update_completions(should_open_completions, handled_position);
    }

    pub fn draw(&mut self, ui: &mut UiHandle, config: &Config) {
        let is_focused = self.widget.is_focused(ui);
        let gfx = ui.gfx();

        for (i, pane) in self.panes.iter_mut().enumerate() {
            let is_focused = is_focused && i == self.focused_pane_index;

            pane.draw(None, &mut self.doc_list, config, gfx, is_focused);
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
        should_open_completions: bool,
        handled_position: Option<Position>,
    ) {
        let position = self.get_cursor_position();

        let is_position_different = position != handled_position;

        if should_open_completions || is_position_different {
            self.completion_prefix.clear();

            Self::clear_completions(
                &mut self.completion_result_list,
                &mut self.completion_result_pool,
            );
        }

        if !should_open_completions {
            return;
        }

        let pane = &mut self.panes[self.focused_pane_index];

        let Some((_, doc)) = pane.get_tab_with_data(pane.focused_tab_index(), &self.doc_list)
        else {
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

    fn get_cursor_position(&self) -> Option<Position> {
        let pane = &self.panes[self.focused_pane_index];

        pane.get_tab_with_data(pane.focused_tab_index(), &self.doc_list)
            .map(|(_, doc)| doc.get_cursor(CursorIndex::Main).position)
    }

    fn is_cursor_visible(&self, gfx: &Gfx) -> bool {
        let pane = &self.panes[self.focused_pane_index];

        let Some((tab, doc)) = pane.get_tab_with_data(pane.focused_tab_index(), &self.doc_list)
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

    fn add_pane(&mut self, line_pool: &mut LinePool) {
        let pane = EditorPane::new(&mut self.doc_list, line_pool);

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
        confirm_close_all(&mut self.doc_list, "exiting", config, line_pool, time);
    }

    pub fn get_focused_pane_and_doc_list(&mut self) -> (&mut EditorPane, &mut SlotList<Doc>) {
        (&mut self.panes[self.focused_pane_index], &mut self.doc_list)
    }
}
