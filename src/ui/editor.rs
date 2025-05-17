use std::{
    env::{current_dir, set_current_dir},
    path::{Path, PathBuf},
};

use completion_list::{CompletionList, CompletionListResult};
use doc_io::confirm_close_all;
use editor_pane::EditorPane;
use examine_popup::ExaminePopup;
use signature_help_popup::SignatureHelpPopup;

use crate::{
    ctx::Ctx,
    geometry::{position::Position, rect::Rect},
    input::{
        action::{action_keybind, action_name},
        mods::Mods,
    },
    lsp::{
        types::{DecodedEditList, Hover},
        uri::uri_to_path,
    },
    platform::{
        dialog::{find_file, message, FindFileKind, MessageKind},
        file_watcher::FileWatcher,
    },
    pool::Pooled,
    text::{
        cursor_index::CursorIndex,
        doc::{Doc, DocKind},
    },
};

use super::{
    core::{Ui, WidgetId},
    slot_list::SlotList,
    tab::Tab,
    widget_list::WidgetList,
};

pub mod completion_list;
mod doc_io;
pub mod editor_pane;
mod examine_popup;
mod signature_help_popup;

pub struct Editor {
    doc_list: SlotList<Doc>,
    // There should always be at least one pane.
    panes: WidgetList<EditorPane>,
    current_dir: Option<PathBuf>,

    handled_position: Option<Position>,
    handled_path: Option<Pooled<PathBuf>>,

    examine_popup: ExaminePopup,
    pub signature_help_popup: SignatureHelpPopup,
    pub completion_list: CompletionList,
    widget_id: WidgetId,
}

impl Editor {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        let widget_id = ui.new_widget(parent_id, Default::default());

        let mut editor = Self {
            doc_list: SlotList::new(),
            panes: WidgetList::new(|pane| pane.widget_id()),
            current_dir: current_dir().ok(),

            handled_position: None,
            handled_path: None,

            examine_popup: ExaminePopup::new(widget_id, ui),
            signature_help_popup: SignatureHelpPopup::new(widget_id, ui),
            completion_list: CompletionList::new(widget_id, ui),
            widget_id,
        };

        editor.add_pane(ui);

        editor
    }

    pub fn is_animating(&self) -> bool {
        self.completion_list.is_animating() || self.panes.iter().any(|pane| pane.is_animating())
    }

    pub fn layout(&mut self, bounds: Rect, ctx: &mut Ctx) {
        ctx.ui.widget_mut(self.widget_id).bounds = bounds;

        let mut pane_bounds = bounds;
        pane_bounds.width = (pane_bounds.width / self.panes.len() as f32).ceil();

        for pane in self.panes.iter_mut() {
            pane.layout(pane_bounds, &mut self.doc_list, ctx);
            pane_bounds.x += pane_bounds.width;
        }

        let focused_pane = self.panes.get_last_focused(ctx.ui).unwrap();

        let Some((tab, doc)) =
            focused_pane.get_tab_with_data(focused_pane.focused_tab_index(), &self.doc_list)
        else {
            return;
        };

        let cursor_position = doc.cursor(CursorIndex::Main).position;
        let cursor_visual_position = doc
            .position_to_visual(cursor_position, tab.camera.position().floor(), ctx.gfx)
            .offset_by(tab.doc_bounds());

        self.completion_list.layout(cursor_visual_position, ctx);
        self.examine_popup.layout(tab, doc, ctx);
        self.signature_help_popup.layout(tab, doc, ctx);
    }

    pub fn update(&mut self, file_watcher: &mut FileWatcher, ctx: &mut Ctx) {
        self.panes.update(ctx.ui);
        self.reload_changed_files(file_watcher, ctx);

        let pane = self.panes.get_last_focused_mut(ctx.ui).unwrap();
        let focused_tab_index = pane.focused_tab_index();

        let doc = pane
            .get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
            .map(|(_, doc)| doc);

        let signature_help_triggers = SignatureHelpPopup::get_triggers(self.widget_id, doc, ctx);

        self.handle_actions(ctx);

        self.pre_pane_update(ctx);

        let pane = self.panes.get_last_focused_mut(ctx.ui).unwrap();
        pane.update(&mut self.doc_list, ctx);
        self.panes
            .remove_excess(ctx.ui, |pane| pane.tabs.is_empty());

        self.post_pane_update(signature_help_triggers, ctx);

        if !ctx.ui.is_in_focused_hierarchy(self.widget_id) {
            self.examine_popup.clear(ctx.ui);
            self.signature_help_popup.clear(ctx.ui);
            self.completion_list.clear();
        }
    }

    fn handle_actions(&mut self, ctx: &mut Ctx) {
        let mut action_handler = ctx.ui.action_handler(self.widget_id, ctx.window);

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
                action_name!(NewPane) => self.add_pane(ctx.ui),
                action_name!(ClosePane) => self.close_pane(ctx),
                action_name!(PreviousPane) => self.panes.focus_previous(ctx.ui),
                action_name!(NextPane) => self.panes.focus_next(ctx.ui),
                action_name!(PreviousTab) => {
                    let pane = self.panes.get_last_focused(ctx.ui).unwrap();

                    if pane.focused_tab_index() == 0 {
                        self.panes.focus_previous(ctx.ui);
                    } else {
                        action_handler.unprocessed(ctx.window, action);
                    }
                }
                action_name!(NextTab) => {
                    let pane = self.panes.get_last_focused(ctx.ui).unwrap();

                    if pane.focused_tab_index() == pane.tabs.len() - 1 {
                        self.panes.focus_next(ctx.ui);
                    } else {
                        action_handler.unprocessed(ctx.window, action);
                    }
                }
                action_keybind!(key: Escape, mods: Mods::NONE) => {
                    if self.signature_help_popup.is_open() {
                        self.signature_help_popup.clear(ctx.ui);
                    } else if self.examine_popup.is_open() {
                        self.examine_popup.clear(ctx.ui);
                    } else {
                        action_handler.unprocessed(ctx.window, action);
                    }
                }
                action_name!(Examine) => {
                    self.signature_help_popup.clear(ctx.ui);

                    let pane = self.panes.get_last_focused_mut(ctx.ui).unwrap();
                    let focused_tab_index = pane.focused_tab_index();

                    if let Some((_, doc)) =
                        pane.get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
                    {
                        self.examine_popup.open(doc, ctx);
                    }
                }
                _ => action_handler.unprocessed(ctx.window, action),
            }
        }
    }

    fn pre_pane_update(&mut self, ctx: &mut Ctx) {
        let is_cursor_visible = self.is_cursor_visible(ctx);
        let pane = self.panes.get_last_focused_mut(ctx.ui).unwrap();
        let focused_tab_index = pane.focused_tab_index();

        let Some((_, doc)) = pane.get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
        else {
            return;
        };

        let result = self.completion_list.update(doc, is_cursor_visible, ctx);

        self.lsp_handle_completion_list_result(result, ctx);
    }

    fn post_pane_update(
        &mut self,
        signature_help_triggers: (Option<char>, Option<char>),
        ctx: &mut Ctx,
    ) {
        let pane = self.panes.get_last_focused_mut(ctx.ui).unwrap();
        let focused_tab_index = pane.focused_tab_index();

        let Some((_, doc)) = pane.get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
        else {
            self.signature_help_popup.clear(ctx.ui);
            self.completion_list.clear();

            return;
        };

        let position = doc.cursor(CursorIndex::Main).position;
        let is_position_different = Some(position) != self.handled_position;
        let is_path_different =
            self.handled_path.as_ref().map(|path| path.as_path()) != doc.path().some_path();
        let did_cursor_move = is_position_different || is_path_different;

        self.signature_help_popup
            .update(signature_help_triggers, doc, ctx);

        self.completion_list
            .update_results(did_cursor_move, doc, ctx);

        self.examine_popup.update(did_cursor_move, doc, ctx);

        self.handled_position = Some(position);
        self.handled_path = doc.path().some().cloned();
    }

    pub fn update_camera(&mut self, ctx: &mut Ctx, dt: f32) {
        for pane in self.panes.iter_mut() {
            pane.update_camera(&mut self.doc_list, ctx, dt);
        }

        self.completion_list.update_camera(ctx.ui, dt);
    }

    pub fn lsp_handle_completion_list_result(
        &mut self,
        result: Option<CompletionListResult>,
        ctx: &mut Ctx,
    ) -> Option<()> {
        let result = result?;

        self.lsp_apply_edit_lists(result.edit_lists, ctx);

        let command = result.command?;
        let (_, doc) = self.get_focused_tab_and_doc_mut(ctx.ui)?;
        let language_server = doc.get_language_server_mut(ctx)?;

        language_server.execute_command(&command.command, &command.arguments);

        Some(())
    }

    pub fn lsp_apply_edit_lists(
        &mut self,
        edit_lists: Vec<DecodedEditList>,
        ctx: &mut Ctx,
    ) -> Option<()> {
        for mut edit_list in edit_lists {
            let path = uri_to_path(&edit_list.uri)?;

            self.with_doc(path, ctx, |doc, ctx| {
                let edits = &mut edit_list.edits;

                doc.lsp_apply_edit_list(edits, ctx);
            });
        }

        Some(())
    }

    pub fn lsp_set_hover(
        &mut self,
        hover: Option<Hover>,
        path: &Pooled<PathBuf>,
        ui: &mut Ui,
    ) -> Option<()> {
        let doc = Self::find_doc(&self.doc_list, path)?;

        self.examine_popup.lsp_set_hover(hover, doc, ui);

        Some(())
    }

    pub fn with_doc(
        &mut self,
        path: Pooled<PathBuf>,
        ctx: &mut Ctx,
        mut doc_fn: impl FnMut(&mut Doc, &mut Ctx),
    ) {
        let doc = self.find_doc_mut(&path);

        let mut loaded_doc = None;

        let doc = doc.or_else(|| {
            loaded_doc = Some(Doc::new(Some(path), None, DocKind::Output));

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

    fn find_doc<'a>(doc_list: &'a SlotList<Doc>, path: &Path) -> Option<&'a Doc> {
        doc_list
            .iter()
            .flatten()
            .find(|doc| doc.path().some_path() == Some(path))
    }

    pub fn find_doc_mut(&mut self, path: &Path) -> Option<&mut Doc> {
        self.doc_list
            .iter_mut()
            .flatten()
            .find(|doc| doc.path().some_path() == Some(path))
    }

    // Necessary when syntax highlighting rules change.
    pub fn clear_doc_highlights(&mut self) {
        for doc in self.doc_list.iter_mut().flatten() {
            doc.clear_highlights();
        }
    }

    fn reload_changed_files(&mut self, file_watcher: &mut FileWatcher, ctx: &mut Ctx) {
        let changed_files = file_watcher.changed_files();

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

    pub fn draw(&mut self, ctx: &mut Ctx) {
        for pane in self.panes.iter_mut() {
            pane.draw(None, &mut self.doc_list, ctx);
        }

        if !self.is_cursor_visible(ctx) {
            return;
        }

        self.completion_list.draw(ctx);

        if self.signature_help_popup.is_open() {
            self.signature_help_popup.draw(ctx);
        } else if self.examine_popup.is_open() {
            self.examine_popup.draw(ctx);
        }
    }

    // TODO: Get rid of these? Or name them ..._last_focused_...
    pub fn get_focused_tab_and_doc_mut(&mut self, ui: &Ui) -> Option<(&mut Tab, &mut Doc)> {
        let pane = self.panes.get_last_focused_mut(ui).unwrap();
        // TODO: We have this pattern a lot, pane should just offer get_focused_tab_with_data(_mut) and we can simply all these usages.
        let focused_tab_index = pane.focused_tab_index();

        pane.get_tab_with_data_mut(focused_tab_index, &mut self.doc_list)
    }

    pub fn get_focused_tab_and_doc(&self, ui: &Ui) -> Option<(&Tab, &Doc)> {
        let pane = self.panes.get_last_focused(ui).unwrap();
        let focused_tab_index = pane.focused_tab_index();

        pane.get_tab_with_data(focused_tab_index, &self.doc_list)
    }

    fn is_cursor_visible(&self, ctx: &mut Ctx) -> bool {
        let pane = self.panes.get_last_focused(ctx.ui).unwrap();

        let Some((tab, doc)) = pane.get_tab_with_data(pane.focused_tab_index(), &self.doc_list)
        else {
            return false;
        };

        let cursor_position = doc.cursor(CursorIndex::Main).position;
        let cursor_visual_position = doc
            .position_to_visual(cursor_position, tab.camera.position(), ctx.gfx)
            .shift_y(ctx.gfx.line_height())
            .offset_by(tab.doc_bounds());

        tab.doc_bounds().contains_position(cursor_visual_position)
    }

    fn add_pane(&mut self, ui: &mut Ui) {
        let pane = EditorPane::new(&mut self.doc_list, self.widget_id, ui);

        self.panes.add(pane, ui);
    }

    fn close_pane(&mut self, ctx: &mut Ctx) {
        if self.panes.len() == 1 {
            return;
        }

        if !self
            .panes
            .get_last_focused_mut(ctx.ui)
            .unwrap()
            .close_all_tabs(&mut self.doc_list, ctx)
        {
            return;
        }

        self.panes.remove(ctx.ui);
    }

    pub fn on_close(&mut self, ctx: &mut Ctx) {
        confirm_close_all(&mut self.doc_list, "exiting", ctx);
    }

    pub fn last_focused_pane_and_doc_list(&self, ui: &Ui) -> (&EditorPane, &SlotList<Doc>) {
        (self.panes.get_last_focused(ui).unwrap(), &self.doc_list)
    }

    pub fn last_focused_pane_and_doc_list_mut(
        &mut self,
        ui: &Ui,
    ) -> (&mut EditorPane, &mut SlotList<Doc>) {
        (
            self.panes.get_last_focused_mut(ui).unwrap(),
            &mut self.doc_list,
        )
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
