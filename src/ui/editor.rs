use std::path::Path;

use completion_list::CompletionList;
use doc_io::confirm_close_all;
use editor_pane::EditorPane;

use crate::{
    ctx::Ctx,
    geometry::{rect::Rect, sides::Sides, visual_position::VisualPosition},
    input::{action::action_name, mods::Mods, mouse_button::MouseButton, mousebind::Mousebind},
    platform::{file_watcher::FileWatcher, gfx::Gfx},
    text::{cursor_index::CursorIndex, doc::Doc, line_pool::LinePool},
};

use super::{
    core::{Ui, Widget},
    focus_list::FocusList,
    slot_list::SlotList,
    tab::Tab,
};

pub mod completion_list;
mod doc_io;
pub mod editor_pane;

pub struct Editor {
    doc_list: SlotList<Doc>,
    // There should always be at least one pane.
    panes: FocusList<EditorPane>,

    pub completion_list: CompletionList,
    pub widget: Widget,
}

impl Editor {
    pub fn new(ui: &mut Ui, line_pool: &mut LinePool) -> Self {
        let mut editor = Self {
            doc_list: SlotList::new(),
            panes: FocusList::new(),
            completion_list: CompletionList::new(),

            widget: Widget::new(ui, true),
        };

        editor.add_pane(line_pool);

        editor
    }

    pub fn is_animating(&self) -> bool {
        self.completion_list.is_animating() || self.panes.iter().any(|pane| pane.is_animating())
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

        self.completion_list.layout(cursor_visual_position, gfx);
        self.widget.layout(&[bounds, self.completion_list.bounds()]);
    }

    pub fn update(&mut self, ui: &mut Ui, file_watcher: &mut FileWatcher, ctx: &mut Ctx) {
        self.reload_changed_files(file_watcher, ctx);

        let mut mousebind_handler = ui.get_mousebind_handler(&self.widget, ctx.window);

        while let Some(mousebind) = mousebind_handler.next(ctx.window) {
            let visual_position =
                VisualPosition::new(mousebind.x, mousebind.y).unoffset_by(self.widget.bounds());

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: Mods::NONE,
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

        let is_cursor_visible = self.is_cursor_visible(ctx.gfx);
        let pane = self.panes.get_focused_mut().unwrap();
        let focused_tab_index = pane.focused_tab_index();

        let handled_position = if let Some((_, doc)) =
            pane.get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
        {
            self.completion_list
                .update(ui, &self.widget, doc, is_cursor_visible, ctx);

            Some(doc.get_cursor(CursorIndex::Main).position)
        } else {
            None
        };

        let pane = self.panes.get_focused_mut().unwrap();

        pane.update(&self.widget, ui, &mut self.doc_list, ctx);

        self.panes.remove_excess(|pane| pane.tabs.is_empty());

        let pane = self.panes.get_focused_mut().unwrap();
        let focused_tab_index = pane.focused_tab_index();

        if let Some((_, doc)) = pane.get_tab_with_data_mut(focused_tab_index, &mut self.doc_list) {
            self.completion_list
                .update_results(doc, handled_position, ctx);
        } else {
            self.completion_list.clear();
        }
    }

    pub fn update_camera(&mut self, ui: &mut Ui, ctx: &mut Ctx, dt: f32) {
        for pane in self.panes.iter_mut() {
            pane.update_camera(&self.widget, ui, &mut self.doc_list, ctx, dt);
        }

        self.completion_list.update_camera(dt);
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
            self.completion_list.draw(ctx);
            self.draw_diagnostic_popup(ctx);
        }
    }

    fn draw_diagnostic_popup(&self, ctx: &mut Ctx) -> Option<()> {
        let (tab, doc) = self.get_focused_tab_and_doc()?;

        let position = doc.get_cursor(CursorIndex::Main).position;

        for language_server in ctx.lsp.iter_servers_mut() {
            for diagnostic in language_server.get_diagnostics_mut(doc) {
                if !diagnostic.is_visible() {
                    continue;
                }

                let (start, end) = diagnostic.range;

                if position < start || position > end {
                    continue;
                }

                let gfx = &mut ctx.gfx;
                let theme = &ctx.config.theme;

                let mut popup_bounds = Rect::ZERO;

                for line in diagnostic.message.lines() {
                    popup_bounds.height += gfx.line_height();

                    let line_width = gfx.measure_text(line) as f32 * gfx.glyph_width();
                    popup_bounds.width = popup_bounds.width.max(line_width);
                }

                let margin = gfx.glyph_width();
                popup_bounds = popup_bounds.add_margin(margin);

                let mut visual_start = doc.position_to_visual(start, tab.camera.position(), gfx);
                visual_start = visual_start.offset_by(tab.doc_bounds());

                popup_bounds.x += visual_start.x;
                popup_bounds.y = visual_start.y - popup_bounds.height;

                if popup_bounds.right() > gfx.width() - margin {
                    popup_bounds.x -= popup_bounds.right() - (gfx.width() - margin);
                }

                popup_bounds.x = popup_bounds.x.max(margin);

                gfx.begin(Some(popup_bounds));

                gfx.add_bordered_rect(
                    popup_bounds.unoffset_by(popup_bounds),
                    Sides::ALL,
                    theme.background,
                    theme.border,
                );

                for (y, line) in diagnostic.message.lines().enumerate() {
                    let y = y as f32 * gfx.line_height() + gfx.line_padding() + margin;

                    gfx.add_text(line, margin, y, theme.normal);
                }

                gfx.end();

                return Some(());
            }
        }

        Some(())
    }

    pub fn get_focused_tab_and_doc(&self) -> Option<(&Tab, &Doc)> {
        let pane = self.panes.get_focused().unwrap();
        let focused_tab_index = pane.focused_tab_index();

        pane.get_tab_with_data(focused_tab_index, &self.doc_list)
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
