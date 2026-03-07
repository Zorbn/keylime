use std::{
    env::set_current_dir,
    io,
    path::{Path, PathBuf},
};

use completion_list::{CompletionList, CompletionListResult};
use cursor_history::CursorHistory;
use doc_io::confirm_close_all;
use editor_pane::EditorPane;
use examine_popup::ExaminePopup;
use signature_help_popup::SignatureHelpPopup;

use crate::{
    ctx::Ctx,
    geometry::position::Position,
    input::{
        action::{action_keybind, action_name},
        mods::Mods,
    },
    lsp::{
        types::{DecodedCompletionItem, DecodedEditList, DecodedHover},
        uri::uri_to_path,
    },
    normalizable::Normalizable,
    platform::{
        dialog::{find_file, message, FindFileKind, MessageKind},
        file_watcher::FileWatcher,
    },
    pool::Pooled,
    text::{
        cursor_index::CursorIndex,
        doc::{Doc, DocFlags},
    },
    ui::msg::Msg,
};

use super::{
    core::{Ui, WidgetId},
    pane_list::PaneList,
    slot_list::{SlotId, SlotList},
};

pub mod completion_list;
mod cursor_history;
mod doc_io;
pub mod editor_pane;
mod examine_popup;
mod signature_help_popup;

pub struct Editor {
    doc_list: SlotList<Doc>,
    panes: PaneList<EditorPane, Doc>,

    handled_position: Option<Position>,
    handled_doc_id: Option<SlotId>,
    cursor_history: CursorHistory,

    hover_timer: f32,

    examine_popup: ExaminePopup,
    pub signature_help_popup: SignatureHelpPopup,
    pub completion_list: CompletionList,
    widget_id: WidgetId,
}

impl Editor {
    const HOVER_TIME: f32 = 0.5;

    pub fn new(parent_id: WidgetId, ctx: &mut Ctx) -> Self {
        let widget_id = ctx.ui.new_widget(parent_id, Default::default());

        let mut editor = Self {
            doc_list: SlotList::new(),
            panes: PaneList::new(widget_id, ctx.ui),

            handled_position: None,
            handled_doc_id: None,
            cursor_history: CursorHistory::new(),

            hover_timer: 0.0,

            examine_popup: ExaminePopup::new(widget_id, ctx),
            signature_help_popup: SignatureHelpPopup::new(widget_id, ctx),
            completion_list: CompletionList::new(widget_id, ctx),
            widget_id,
        };

        editor.add_pane(ctx);

        editor
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.completion_list.is_animating(ctx)
            || self.signature_help_popup.is_animating(ctx)
            || self.examine_popup.is_animating(ctx)
            || self.panes.is_animating(ctx)
            || self.hover_timer > 0.0
    }

    pub fn receive_msgs(&mut self, ctx: &mut Ctx) {
        while let Some(msg) = ctx.ui.msg(self.widget_id) {
            match msg {
                Msg::ShowCompletions => {
                    let pane = self.panes.get_last_focused_mut(ctx.ui).unwrap();

                    if let Some((tab, _)) =
                        pane.get_focused_tab_with_data_mut(&mut self.doc_list, ctx.ui)
                    {
                        self.completion_list.show(tab.widget_id(), ctx);
                    }
                }
                Msg::HideCompletions => self.completion_list.hide(ctx),
                Msg::TriggerSignatureHelp {
                    trigger_char,
                    is_retrigger,
                } => {
                    let pane = self.panes.get_last_focused_mut(ctx.ui).unwrap();

                    if let Some((tab, doc)) =
                        pane.get_focused_tab_with_data_mut(&mut self.doc_list, ctx.ui)
                    {
                        self.signature_help_popup.show(
                            doc,
                            tab.widget_id(),
                            trigger_char,
                            is_retrigger,
                            ctx,
                        )
                    }
                }
                Msg::HideEditorPopups => {
                    self.signature_help_popup.hide(ctx.ui);
                    self.examine_popup.hide(ctx.ui);
                    self.completion_list.hide(ctx)
                }
                Msg::TabHoverChanged => self.hover_timer = Self::HOVER_TIME,
                Msg::Action(action_name!(OpenFolder)) => {
                    if let Ok(path) = find_file(FindFileKind::OpenFolder, ctx.window) {
                        if let Err(err) = Self::open_folder(&path, ctx) {
                            message(
                                "Error Opening Folder",
                                &err.to_string(),
                                MessageKind::Ok,
                                ctx.window,
                            );
                        }
                    }
                }
                Msg::Action(action_name!(NewPane)) => self.add_pane(ctx),
                Msg::Action(action_name!(ClosePane)) => self.close_pane(ctx),
                Msg::Action(action_keybind!(key: Escape, mods: Mods::NONE)) => {
                    if self.signature_help_popup.is_open() {
                        self.signature_help_popup.hide(ctx.ui);
                    } else if self.examine_popup.is_open() {
                        self.examine_popup.hide(ctx.ui);
                    } else {
                        ctx.ui.skip(self.widget_id, msg);
                    }
                }
                Msg::Action(action_name!(Examine)) => {
                    self.signature_help_popup.hide(ctx.ui);

                    let pane = self.panes.get_last_focused_mut(ctx.ui).unwrap();

                    if let Some((tab, doc)) =
                        pane.get_focused_tab_with_data_mut(&mut self.doc_list, ctx.ui)
                    {
                        let position = doc.cursor(CursorIndex::Main).position;
                        self.examine_popup
                            .show(position, tab.widget_id(), true, doc, ctx);
                    }
                }
                Msg::Action(action_name!(UndoCursorPosition)) => {
                    self.cursor_history
                        .undo(&mut self.panes, &mut self.doc_list, ctx);
                }
                Msg::Action(action_name!(RedoCursorPosition)) => {
                    self.cursor_history
                        .redo(&mut self.panes, &mut self.doc_list, ctx);
                }
                _ => ctx.ui.skip(self.widget_id, msg),
            }
        }

        self.panes.receive_msgs(&mut self.doc_list, ctx);
        self.signature_help_popup.receive_msgs(ctx);
        self.examine_popup.receive_msgs(ctx);

        let pane = self.panes.get_last_focused_mut(ctx.ui).unwrap();

        let doc = pane
            .get_focused_tab_with_data_mut(&mut self.doc_list, ctx.ui)
            .map(|(_, doc)| doc);

        let result = self.completion_list.receive_msgs(doc, ctx);
        self.lsp_handle_completion_list_result(result, ctx);
    }

    pub fn update(&mut self, file_watcher: &mut FileWatcher, ctx: &mut Ctx, dt: f32) {
        self.panes.update(&mut self.doc_list, ctx, dt);
        self.reload_changed_files(file_watcher, ctx);

        self.update_hover(ctx, dt);

        self.panes.remove_excess(ctx.ui, |pane| !pane.has_tabs());

        let is_cursor_visible = self.is_cursor_visible(ctx);

        ctx.ui
            .set_shown(self.completion_list.widget_id(), is_cursor_visible);
        ctx.ui
            .set_shown(self.signature_help_popup.widget_id(), is_cursor_visible);

        let pane = self.panes.get_last_focused_mut(ctx.ui).unwrap();

        let Some((tab, doc)) = pane.get_focused_tab_with_data_mut(&mut self.doc_list, ctx.ui)
        else {
            self.signature_help_popup.hide(ctx.ui);
            self.completion_list.hide(ctx);

            return;
        };

        self.completion_list.update(tab, doc, ctx, dt);

        let doc_id = tab.data_id();
        let position = doc.cursor(CursorIndex::Main).position;

        self.cursor_history
            .update(self.handled_doc_id, doc_id, self.handled_position, position);

        self.signature_help_popup.update(tab, doc, ctx, dt);
        self.examine_popup.update(tab, doc, ctx, dt);

        self.handled_position = Some(position);
        self.handled_doc_id = Some(doc_id);
    }

    fn update_hover(&mut self, ctx: &mut Ctx, dt: f32) {
        let last_hover_timer = self.hover_timer;
        self.hover_timer = (self.hover_timer - dt).max(0.0);

        if last_hover_timer == 0.0
            || self.hover_timer > 0.0
            || ctx.ui.is_hovered(self.examine_popup.widget_id())
        {
            return;
        }

        let pane = self.panes.get_hovered_mut(ctx.ui);

        let Some((tab, doc)) =
            pane.and_then(|pane| pane.get_focused_tab_with_data_mut(&mut self.doc_list, ctx.ui))
        else {
            self.examine_popup.hide(ctx.ui);
            return;
        };

        if let Some(position) =
            tab.visual_to_position_unclamped(ctx.window.mouse_position(), doc, ctx.ui, ctx.gfx)
        {
            self.examine_popup
                .show(position, tab.widget_id(), false, doc, ctx);
        } else {
            self.examine_popup.hide(ctx.ui);
        }
    }

    pub fn lsp_handle_completion_list_result(
        &mut self,
        result: Option<CompletionListResult>,
        ctx: &mut Ctx,
    ) -> Option<()> {
        let result = result?;

        self.lsp_apply_edit_lists(result.edit_lists, ctx);

        let command = result.command?;
        let pane = self.panes.get_last_focused_mut(ctx.ui).unwrap();
        let (_, doc) = pane.get_focused_tab_with_data_mut(&mut self.doc_list, ctx.ui)?;
        let language_server = doc.get_language_server_mut(ctx)?;

        language_server.execute_command(&command.command, &command.arguments);

        Some(())
    }

    pub fn lsp_apply_edit_lists(
        &mut self,
        edit_lists: Vec<DecodedEditList>,
        ctx: &mut Ctx,
    ) -> Option<()> {
        for edit_list in edit_lists {
            let path = uri_to_path(&edit_list.uri)?;

            self.with_doc(path, ctx, |doc, ctx| {
                let edits = &mut edit_list.edits(doc);

                doc.lsp_apply_edit_list(edits, ctx);
            });
        }

        Some(())
    }

    pub fn lsp_update_completion_results(
        &mut self,
        items: Vec<DecodedCompletionItem>,
        needs_resolve: bool,
        doc_id: SlotId,
        ctx: &mut Ctx,
    ) -> Option<()> {
        let doc = self.doc_list.get(doc_id)?;

        self.completion_list
            .lsp_update_completion_results(items, needs_resolve, doc, ctx);

        Some(())
    }

    pub fn lsp_set_hover(
        &mut self,
        hover: Option<DecodedHover>,
        doc_id: SlotId,
        ctx: &mut Ctx,
    ) -> Option<()> {
        let doc = self.doc_list.get(doc_id)?;

        self.examine_popup.lsp_set_hover(hover, doc, ctx);

        Some(())
    }

    pub fn with_doc(
        &mut self,
        path: Pooled<PathBuf>,
        ctx: &mut Ctx,
        on_doc: impl FnOnce(&mut Doc, &mut Ctx),
    ) {
        let doc = self.find_doc_mut(&path);

        let mut loaded_doc = None;

        let doc = doc.or_else(|| {
            loaded_doc = Some(Doc::new(Some(path), None, DocFlags::RAW));

            let doc = loaded_doc.as_mut()?;
            doc.load(ctx).ok()?;

            Some(doc)
        });

        if let Some(doc) = doc {
            on_doc(doc, ctx);
        };

        if let Some(mut doc) = loaded_doc {
            let _ = doc.save(None, ctx);
            doc.clear(ctx);
        }
    }

    pub fn find_doc_mut(&mut self, path: &Path) -> Option<&mut Doc> {
        self.doc_list
            .iter_mut()
            .find(|doc| doc.path().some_path() == Some(path))
    }

    pub fn find_doc_with_id_mut(&mut self, path: &Path) -> Option<(SlotId, &mut Doc)> {
        self.doc_list
            .enumerate_mut()
            .find(|(_, doc)| doc.path().some_path() == Some(path))
    }

    // Necessary when syntax highlighting rules change.
    pub fn clear_doc_highlights(&mut self) {
        for doc in self.doc_list.iter_mut() {
            doc.clear_highlights();
        }
    }

    fn reload_changed_files(&mut self, file_watcher: &mut FileWatcher, ctx: &mut Ctx) {
        let current_dir = ctx.current_dir.clone();
        let changed_files = file_watcher.changed_files(&current_dir);

        for path in changed_files {
            for doc in self.doc_list.iter_mut() {
                if doc.path().some() != Some(&path) {
                    continue;
                }

                if doc.is_change_unexpected() {
                    let _ = doc.reload(ctx);
                }

                break;
            }
        }
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        self.panes.draw(None, &mut self.doc_list, ctx);

        self.completion_list.draw(ctx);

        if self.signature_help_popup.is_open() {
            self.signature_help_popup.draw(ctx);
        } else if self.examine_popup.is_open() {
            self.examine_popup.draw(ctx);
        }
    }

    fn is_cursor_visible(&self, ctx: &mut Ctx) -> bool {
        let pane = self.panes.get_last_focused(ctx.ui).unwrap();

        let Some((tab, doc)) = pane.get_focused_tab_with_data(&self.doc_list, ctx.ui) else {
            return false;
        };

        let doc_bounds = tab.doc_bounds(ctx.ui);

        let cursor_position = doc.cursor(CursorIndex::Main).position;
        let cursor_visual_position = doc
            .position_to_visual(cursor_position, tab.camera.position(), ctx.gfx)
            .shift_y(ctx.gfx.line_height())
            .offset_by(doc_bounds);

        doc_bounds.contains_position(cursor_visual_position)
    }

    fn add_pane(&mut self, ctx: &mut Ctx) {
        let pane = EditorPane::new(&mut self.doc_list, self.panes.widget_id(), ctx);

        self.panes.add(pane, ctx.ui);
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

        self.panes.remove_focused(ctx.ui);
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

    pub fn open_folder(path: &Path, ctx: &mut Ctx) -> io::Result<()> {
        let new_current_dir = path.normalized(ctx.current_dir)?;

        ctx.current_dir.clear();
        ctx.current_dir.push(new_current_dir);

        set_current_dir(&ctx.current_dir)?;

        ctx.lsp.clear();

        Ok(())
    }

    pub fn files(&self) -> impl Iterator<Item = &Path> {
        self.doc_list.iter().filter_map(|doc| doc.path().on_drive())
    }
}
