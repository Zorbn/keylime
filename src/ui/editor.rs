use std::{
    env::{current_dir, set_current_dir},
    path::{Path, PathBuf},
};

use completion_list::{CompletionList, CompletionListResult};
use doc_io::confirm_close_all;
use editor_pane::EditorPane;
use signature_help_popup::SignatureHelpPopup;

use crate::{
    ctx::Ctx,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    input::{
        action::{action_keybind, action_name},
        mods::Mods,
        mouse_button::MouseButton,
        mousebind::{MouseClickKind, Mousebind},
    },
    lsp::{types::EditList, uri::uri_to_path},
    platform::{
        dialog::{find_file, message, FindFileKind, MessageKind},
        file_watcher::FileWatcher,
        gfx::Gfx,
    },
    text::{
        cursor_index::CursorIndex,
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
};

use super::{
    core::{Ui, Widget},
    focus_list::FocusList,
    popup::{draw_popup, PopupAlignment},
    slot_list::SlotList,
    tab::Tab,
};

pub mod completion_list;
mod doc_io;
pub mod editor_pane;
mod signature_help_popup;

pub struct Editor {
    doc_list: SlotList<Doc>,
    // There should always be at least one pane.
    panes: FocusList<EditorPane>,
    current_dir: Option<PathBuf>,

    handled_position: Option<Position>,
    do_show_diagnostic_popup: bool,
    pub signature_help_popup: SignatureHelpPopup,
    pub completion_list: CompletionList,
    pub widget: Widget,
}

impl Editor {
    pub fn new(ui: &mut Ui, line_pool: &mut LinePool) -> Self {
        let mut editor = Self {
            doc_list: SlotList::new(),
            panes: FocusList::new(),
            current_dir: current_dir().ok(),

            handled_position: None,
            do_show_diagnostic_popup: false,
            signature_help_popup: SignatureHelpPopup::new(),
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

        let pane = self.panes.get_focused_mut().unwrap();
        let focused_tab_index = pane.focused_tab_index();

        let doc = pane
            .get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
            .map(|(_, doc)| doc);

        let signature_help_triggers =
            self.signature_help_popup
                .get_triggers(&self.widget, ui, doc, ctx);

        self.handle_mousebinds(ui, ctx);
        self.handle_actions(ui, ctx);

        self.pre_pane_update(ui, ctx);

        let pane = self.panes.get_focused_mut().unwrap();
        pane.update(&self.widget, ui, &mut self.doc_list, ctx);
        self.panes.remove_excess(|pane| pane.tabs.is_empty());

        self.post_pane_update(signature_help_triggers, ctx);

        if !ui.is_focused(&self.widget) {
            self.do_show_diagnostic_popup = false;
            self.completion_list.clear();
            self.signature_help_popup.clear();
        }
    }

    fn handle_mousebinds(&mut self, ui: &mut Ui, ctx: &mut Ctx) {
        let mut mousebind_handler = ui.get_mousebind_handler(&self.widget, ctx.window);

        while let Some(mousebind) = mousebind_handler.next(ctx.window) {
            let visual_position =
                VisualPosition::new(mousebind.x, mousebind.y).unoffset_by(self.widget.bounds());

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: Mods::NONE,
                    kind: MouseClickKind::Press,
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
    }

    fn handle_actions(&mut self, ui: &mut Ui, ctx: &mut Ctx) {
        let mut action_handler = ui.get_action_handler(&self.widget, ctx.window);

        while let Some(action) = action_handler.next(ctx.window) {
            match action {
                action_name!(OpenFolder) => {
                    if let Ok(path) = find_file(FindFileKind::OpenFolder) {
                        if let Err(err) = set_current_dir(path) {
                            message("Error Opening Folder", &err.to_string(), MessageKind::Ok);
                        } else {
                            self.current_dir = current_dir().ok();
                            ctx.lsp.update_current_dir(self.current_dir.clone());
                        }
                    }
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
                action_keybind!(key: Escape, mods: Mods::NONE) => {
                    if let Some((_, doc)) = self.get_focused_tab_and_doc() {
                        let position = doc.get_cursor(CursorIndex::Main).position;

                        if self.signature_help_popup.is_open() {
                            self.signature_help_popup.clear();
                        } else if self.do_show_diagnostic_popup
                            && ctx.lsp.get_diagnostic_at(position, doc).is_some()
                        {
                            self.do_show_diagnostic_popup = false;
                        } else {
                            action_handler.unprocessed(ctx.window, action);
                        }
                    }
                }
                action_name!(ShowDiagnostic) => self.do_show_diagnostic_popup = true,
                _ => action_handler.unprocessed(ctx.window, action),
            }
        }
    }

    fn pre_pane_update(&mut self, ui: &mut Ui, ctx: &mut Ctx) {
        let is_cursor_visible = self.is_cursor_visible(ctx.gfx);
        let pane = self.panes.get_focused_mut().unwrap();
        let focused_tab_index = pane.focused_tab_index();

        self.handled_position = None;

        let Some((_, doc)) = pane.get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
        else {
            return;
        };

        self.handled_position = Some(doc.get_cursor(CursorIndex::Main).position);

        let result = self
            .completion_list
            .update(ui, &self.widget, doc, is_cursor_visible, ctx);

        self.handle_completion_list_result(result, ctx);
    }

    fn post_pane_update(
        &mut self,
        signature_help_triggers: (Option<char>, Option<char>),
        ctx: &mut Ctx,
    ) {
        let pane = self.panes.get_focused_mut().unwrap();
        let focused_tab_index = pane.focused_tab_index();

        let Some((_, doc)) = pane.get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
        else {
            self.signature_help_popup.clear();
            self.completion_list.clear();

            return;
        };

        self.signature_help_popup
            .update(signature_help_triggers, doc, ctx);

        self.completion_list
            .update_results(doc, self.handled_position, ctx);

        let position = doc.get_cursor(CursorIndex::Main).position;

        if self.do_show_diagnostic_popup && ctx.lsp.get_diagnostic_at(position, doc).is_none() {
            self.do_show_diagnostic_popup = false;
        }
    }

    pub fn update_camera(&mut self, ui: &mut Ui, ctx: &mut Ctx, dt: f32) {
        for pane in self.panes.iter_mut() {
            pane.update_camera(&self.widget, ui, &mut self.doc_list, ctx, dt);
        }

        self.completion_list.update_camera(dt);
    }

    pub fn handle_completion_list_result(
        &mut self,
        result: Option<CompletionListResult>,
        ctx: &mut Ctx,
    ) -> Option<()> {
        let result = result?;

        self.apply_edit_lists(result.edit_lists, ctx);

        let command = result.command?;
        let (_, doc) = self.get_focused_tab_and_doc_mut()?;
        let (_, language_server) = doc.get_language_server_mut(ctx)?;

        language_server.execute_command(&command.command, &command.arguments);

        Some(())
    }

    pub fn apply_edit_lists(&mut self, edit_lists: Vec<EditList>, ctx: &mut Ctx) -> Option<()> {
        for mut edit_list in edit_lists {
            let path = uri_to_path(&edit_list.uri, String::new())?;

            self.with_doc(path, ctx, |doc, ctx| {
                let edits = &mut edit_list.edits;

                doc.apply_edit_list(edits, ctx);
            });
        }

        Some(())
    }

    pub fn with_doc(
        &mut self,
        path: PathBuf,
        ctx: &mut Ctx,
        mut doc_fn: impl FnMut(&mut Doc, &mut Ctx),
    ) {
        let doc = self.find_doc_mut(&path);

        let mut loaded_doc = None;

        let doc = doc.or_else(|| {
            loaded_doc = Some(Doc::new(
                Some(path),
                &mut ctx.buffers.lines,
                None,
                DocKind::Output,
            ));

            let doc = loaded_doc.as_mut()?;
            doc.load(ctx).ok()?;

            Some(doc)
        });

        if let Some(doc) = doc {
            doc_fn(doc, ctx);
        };

        if let Some(mut doc) = loaded_doc {
            let _ = doc.save(None, ctx);
            doc.clear(ctx);
        }
    }

    pub fn find_doc_mut(&mut self, path: &Path) -> Option<&mut Doc> {
        self.doc_list
            .iter_mut()
            .flatten()
            .find(|doc| doc.path().on_drive() == Some(path))
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

        if !self.is_cursor_visible(ctx.gfx) {
            return;
        }

        self.completion_list.draw(ctx);

        let Some((tab, doc)) = self.get_focused_tab_and_doc() else {
            return;
        };

        if self.signature_help_popup.is_open() {
            self.signature_help_popup.draw(tab, doc, ctx);
        } else if self.do_show_diagnostic_popup {
            self.draw_diagnostic_popup(tab, doc, ctx);
        }
    }

    fn draw_diagnostic_popup(&self, tab: &Tab, doc: &Doc, ctx: &mut Ctx) -> Option<()> {
        let position = doc.get_cursor(CursorIndex::Main).position;

        if let Some(diagnostic) = ctx.lsp.get_diagnostic_at(position, doc) {
            let gfx = &mut ctx.gfx;
            let theme = &ctx.config.theme;

            let (start, _) = diagnostic.get_visible_range(doc);

            let mut position = doc.position_to_visual(start, tab.camera.position(), gfx);
            position = position.offset_by(tab.doc_bounds());

            draw_popup(
                &diagnostic.message,
                position,
                PopupAlignment::Above,
                theme.normal,
                theme,
                gfx,
            );
        }

        Some(())
    }

    pub fn get_focused_tab_and_doc_mut(&mut self) -> Option<(&mut Tab, &mut Doc)> {
        let pane = self.panes.get_focused_mut().unwrap();
        let focused_tab_index = pane.focused_tab_index();

        pane.get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
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

    pub fn get_focused_pane_and_doc_list(&self) -> (&EditorPane, &SlotList<Doc>) {
        (self.panes.get_focused().unwrap(), &self.doc_list)
    }

    pub fn get_focused_pane_and_doc_list_mut(&mut self) -> (&mut EditorPane, &mut SlotList<Doc>) {
        (self.panes.get_focused_mut().unwrap(), &mut self.doc_list)
    }

    pub fn current_dir(&self) -> Option<&Path> {
        self.current_dir.as_deref()
    }

    pub fn files(&self) -> impl Iterator<Item = &Path> {
        self.doc_list
            .iter()
            .flatten()
            .filter_map(|doc| doc.path().on_drive())
    }
}
