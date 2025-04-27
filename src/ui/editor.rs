use std::path::Path;

use doc_io::confirm_close_all;
use editor_pane::EditorPane;

use crate::{
    ctx::Ctx,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    input::{
        action::{action_keybind, action_name},
        mouse_button::MouseButton,
        mousebind::Mousebind,
    },
    lsp::LspCompletionList,
    platform::{file_watcher::FileWatcher, gfx::Gfx},
    text::{cursor_index::CursorIndex, doc::Doc, grapheme, line_pool::LinePool},
};

use super::{
    core::{Ui, Widget},
    focus_list::FocusList,
    result_list::{ResultList, ResultListInput, ResultListSubmitKind},
    slot_list::SlotList,
};

mod doc_io;
pub mod editor_pane;

const MAX_VISIBLE_COMPLETION_RESULTS: usize = 10;

pub struct Editor {
    doc_list: SlotList<Doc>,
    // There should always be at least one pane.
    panes: FocusList<EditorPane>,

    lsp_completion_result_count: usize,
    completion_result_list: ResultList<String>,
    completion_result_pool: LinePool,
    completion_prefix: String,

    pub widget: Widget,
}

impl Editor {
    pub fn new(ui: &mut Ui, line_pool: &mut LinePool) -> Self {
        let mut editor = Self {
            doc_list: SlotList::new(),
            panes: FocusList::new(),

            lsp_completion_result_count: 0,
            completion_result_list: ResultList::new(MAX_VISIBLE_COMPLETION_RESULTS),
            completion_result_pool: LinePool::new(),
            completion_prefix: String::new(),

            widget: Widget::new(ui, true),
        };

        editor.add_pane(line_pool);

        editor
    }

    pub fn is_animating(&self) -> bool {
        self.completion_result_list.is_animating()
            || self.panes.iter().any(|pane| pane.is_animating())
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &mut Gfx) {
        let mut pane_bounds = bounds;
        pane_bounds.width = (pane_bounds.width / self.panes.len() as f32).ceil();

        for pane in self.panes.iter_mut() {
            pane.layout(pane_bounds, gfx, &mut self.doc_list);
            pane_bounds.x += pane_bounds.width;
        }

        let focused_pane = self.panes.get_focused().unwrap();

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

    pub fn update(&mut self, ui: &mut Ui, file_watcher: &mut FileWatcher, ctx: &mut Ctx) {
        self.reload_changed_files(file_watcher, ctx);

        let mut grapheme_handler = ui.get_grapheme_handler(&self.widget, ctx.window);

        let mut should_open_completions = if grapheme_handler.next(ctx.window).is_some() {
            grapheme_handler.unprocessed(ctx.window);

            true
        } else {
            false
        };

        let mut mousebind_handler = ui.get_mousebind_handler(&self.widget, ctx.window);

        while let Some(mousebind) = mousebind_handler.next(ctx.window) {
            let visual_position =
                VisualPosition::new(mousebind.x, mousebind.y).unoffset_by(self.widget.bounds());

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: 0,
                    is_drag: false,
                    ..
                } => {
                    let index = self
                        .panes
                        .iter()
                        .position(|pane| pane.bounds().contains_position(visual_position));

                    if let Some(index) = index {
                        self.panes.set_focused_index(index);
                    }

                    mousebind_handler.unprocessed(ctx.window, mousebind);
                }
                _ => mousebind_handler.unprocessed(ctx.window, mousebind),
            }
        }

        let mut action_handler = ui.get_action_handler(&self.widget, ctx.window);

        while let Some(action) = action_handler.next(ctx.window) {
            match action {
                action_keybind!(key: Backspace) => {
                    should_open_completions = true;

                    action_handler.unprocessed(ctx.window, action);
                }
                action_name!(NewPane) => self.add_pane(&mut ctx.buffers.lines),
                action_name!(ClosePane) => self.close_pane(ctx),
                action_name!(PreviousPane) => self.panes.focus_previous(),
                action_name!(NextPane) => self.panes.focus_next(),
                action_name!(PreviousTab) => {
                    let pane = self.panes.get_focused().unwrap();

                    if pane.focused_tab_index() == 0 {
                        self.panes.focus_previous();
                    } else {
                        action_handler.unprocessed(ctx.window, action);
                    }
                }
                action_name!(NextTab) => {
                    let pane = self.panes.get_focused().unwrap();

                    if pane.focused_tab_index() == pane.tabs.len() - 1 {
                        self.panes.focus_next();
                    } else {
                        action_handler.unprocessed(ctx.window, action);
                    }
                }
                _ => action_handler.unprocessed(ctx.window, action),
            }
        }

        let are_results_visible = self.is_cursor_visible(ctx.gfx);
        let are_results_focused = !self.completion_result_list.results.is_empty();

        let result_input = self.completion_result_list.update(
            &mut self.widget,
            ui,
            ctx.window,
            are_results_visible,
            are_results_focused,
        );

        match result_input {
            ResultListInput::None => {}
            ResultListInput::Complete
            | ResultListInput::Submit {
                kind: ResultListSubmitKind::Normal,
            } => {
                if let Some(result) = self.completion_result_list.get_selected_result() {
                    let pane = self.panes.get_focused_mut().unwrap();
                    let focused_tab_index = pane.focused_tab_index();

                    if let Some((_, doc)) =
                        pane.get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
                    {
                        doc.insert_at_cursors(&result[self.completion_prefix.len()..], ctx);
                    }
                }

                self.clear_completions();
            }
            ResultListInput::Close => {
                self.clear_completions();
            }
            _ => {}
        }

        let handled_position = self.get_cursor_position();
        let pane = self.panes.get_focused_mut().unwrap();

        pane.update(&mut self.widget, ui, &mut self.doc_list, ctx);

        self.panes.remove_excess(|pane| pane.tabs.is_empty());

        self.update_completions(should_open_completions, handled_position, ctx);
    }

    pub fn update_camera(&mut self, ui: &mut Ui, ctx: &mut Ctx, dt: f32) {
        for pane in self.panes.iter_mut() {
            pane.update_camera(&mut self.widget, ui, &mut self.doc_list, ctx, dt);
        }

        self.completion_result_list.update_camera(dt);
    }

    pub fn lsp_add_completion_results(&mut self, completion_list: &LspCompletionList) {
        self.lsp_clear_completion_results();

        let non_lsp_result_count = self.completion_result_list.results.len();

        // TODO: Make the serde LSP structs used borrowed strings and then here we copy them into pool strings.
        self.completion_result_list.results.splice(
            0..0,
            completion_list
                .items
                .iter()
                .filter(|item| {
                    item.filter_text
                        .as_ref()
                        .unwrap_or(&item.label)
                        .starts_with(&self.completion_prefix)
                })
                .map(|item| {
                    let mut result = self.completion_result_pool.pop();
                    result.push_str(&item.label);
                    result
                }),
        );

        self.lsp_completion_result_count =
            self.completion_result_list.results.len() - non_lsp_result_count;
    }

    fn lsp_clear_completion_results(&mut self) {
        for result in self
            .completion_result_list
            .results
            .drain(..self.lsp_completion_result_count)
        {
            self.completion_result_pool.push(result);
        }

        self.lsp_completion_result_count = 0;
    }

    // Necessary when syntax highlighting rules change.
    pub fn clear_doc_highlights(&mut self) {
        for doc in self.doc_list.iter_mut().flatten() {
            doc.clear_highlights();
        }
    }

    fn reload_changed_files(&mut self, file_watcher: &mut FileWatcher, ctx: &mut Ctx) {
        let changed_files = file_watcher.get_changed_files();

        for path in changed_files {
            for doc in self.doc_list.iter_mut().flatten() {
                if doc.path().some() != Some(path) {
                    continue;
                }

                if doc.is_change_unexpected() {
                    doc.reload(ctx).unwrap();
                }

                break;
            }
        }
    }

    pub fn draw(&mut self, ui: &mut Ui, ctx: &mut Ctx) {
        let is_focused = ui.is_focused(&self.widget);
        let focused_pane_index = self.panes.focused_index();

        for (i, pane) in self.panes.iter_mut().enumerate() {
            let is_focused = is_focused && i == focused_pane_index;

            pane.draw(None, &mut self.doc_list, ctx, is_focused);
        }

        if self.is_cursor_visible(ctx.gfx) {
            self.completion_result_list.draw(ctx, |result| result);
        }
    }

    fn get_completion_prefix<'a>(doc: &'a Doc, gfx: &mut Gfx) -> Option<&'a str> {
        let prefix_end = doc.get_cursor(CursorIndex::Main).position;

        if prefix_end.x == 0 {
            return None;
        }

        let mut prefix_start = prefix_end;

        while prefix_start.x > 0 {
            let next_start = doc.move_position(prefix_start, -1, 0, gfx);

            let grapheme = doc.get_grapheme(next_start);

            if grapheme::is_alphanumeric(grapheme) || grapheme == "_" {
                prefix_start = next_start;
                continue;
            }

            if grapheme::is_whitespace(grapheme) && prefix_start == prefix_end {
                return None;
            }

            break;
        }

        doc.get_line(prefix_end.y)
            .map(|line| &line[prefix_start.x..prefix_end.x])
    }

    fn clear_completions(&mut self) {
        for result in self.completion_result_list.drain() {
            self.completion_result_pool.push(result);
        }

        self.lsp_completion_result_count = 0;
    }

    fn update_completions(
        &mut self,
        should_open_completions: bool,
        handled_position: Option<Position>,
        ctx: &mut Ctx,
    ) {
        let position = self.get_cursor_position();

        let is_position_different = position != handled_position;

        if should_open_completions || is_position_different {
            self.completion_prefix.clear();

            self.clear_completions();
        }

        if !should_open_completions {
            return;
        }

        let pane = self.panes.get_focused_mut().unwrap();
        let focused_tab_index = pane.focused_tab_index();

        let Some((_, doc)) = pane.get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
        else {
            return;
        };

        let Some(prefix) = Self::get_completion_prefix(doc, ctx.gfx) else {
            return;
        };

        self.completion_prefix.push_str(prefix);

        if !prefix.is_empty() {
            doc.tokens().traverse(
                prefix,
                &mut self.completion_result_list.results,
                &mut self.completion_result_pool,
            );
        }

        doc.lsp_completion(doc.get_cursor(CursorIndex::Main).position, ctx);
    }

    fn get_cursor_position(&self) -> Option<Position> {
        let pane = self.panes.get_focused().unwrap();

        pane.get_tab_with_data(pane.focused_tab_index(), &self.doc_list)
            .map(|(_, doc)| doc.get_cursor(CursorIndex::Main).position)
    }

    fn is_cursor_visible(&self, gfx: &mut Gfx) -> bool {
        let pane = self.panes.get_focused().unwrap();

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

    fn add_pane(&mut self, line_pool: &mut LinePool) {
        let pane = EditorPane::new(&mut self.doc_list, line_pool);

        self.panes.add(pane);
    }

    fn close_pane(&mut self, ctx: &mut Ctx) {
        if self.panes.len() == 1 {
            return;
        }

        if !self
            .panes
            .get_focused_mut()
            .unwrap()
            .close_all_tabs(&mut self.doc_list, ctx)
        {
            return;
        }

        self.panes.remove();
    }

    pub fn on_close(&mut self, ctx: &mut Ctx) {
        confirm_close_all(&mut self.doc_list, "exiting", ctx);
    }

    pub fn get_focused_pane_and_doc_list(&mut self) -> (&mut EditorPane, &mut SlotList<Doc>) {
        (self.panes.get_focused_mut().unwrap(), &mut self.doc_list)
    }

    pub fn files(&self) -> impl Iterator<Item = &Path> {
        self.doc_list
            .iter()
            .flatten()
            .filter_map(|doc| doc.path().on_drive())
    }
}
