use std::collections::{hash_map::Entry, HashMap};

use crate::{
    ctx::Ctx,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    lsp::{
        types::{
            Command, DecodedCodeAction, DecodedCodeActionResult, DecodedCompletionItem,
            DecodedEditList, DecodedRange,
        },
        LspSentRequest,
    },
    pool::Pooled,
    text::{compare::score_fuzzy_match, cursor_index::CursorIndex, doc::Doc},
    ui::{
        core::{WidgetId, WidgetSettings},
        popup::{Popup, PopupAlignment},
        result_list::{ResultList, ResultListInput, ResultListSubmitKind},
        tab::Tab,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionResolveState {
    NeedsRequest,
    NeedsResponse,
    Resolved,
}

#[derive(Debug)]
enum CompletionResult {
    SimpleCompletion(Pooled<String>),
    Completion {
        item: DecodedCompletionItem,
        resolve_state: CompletionResolveState,
    },
    Command(Command),
    CodeAction(DecodedCodeAction),
}

impl CompletionResult {
    pub fn label(&self) -> &str {
        match self {
            Self::SimpleCompletion(text) => text,
            Self::Completion { item, .. } => &item.label,
            Self::Command(command) => &command.title,
            Self::CodeAction(code_action) => &code_action.title,
        }
    }
}

#[derive(Debug, Default)]
pub struct CompletionListResult {
    pub edit_lists: Vec<DecodedEditList>,
    pub command: Option<Command>,
}

pub struct CompletionList {
    widget_id: WidgetId,

    needs_results: bool,
    result_list: ResultList<CompletionResult>,
    prefix: String,

    lsp_expected_responses: HashMap<usize, usize>,

    detail_popup: Popup,
    documentation_popup: Popup,
}

impl CompletionList {
    const MAX_VISIBLE_RESULTS: usize = 10;

    pub fn new(parent_id: WidgetId, ctx: &mut Ctx) -> Self {
        let widget_id = ctx.ui.new_widget(
            parent_id,
            WidgetSettings {
                popup: Some(Rect::ZERO),
                wants_msgs: false,
                is_owned_by_parent: false,
                ..Default::default()
            },
        );

        Self {
            widget_id,

            needs_results: false,
            result_list: ResultList::new(widget_id, ctx.ui),
            prefix: String::new(),

            lsp_expected_responses: HashMap::new(),

            detail_popup: Popup::new(widget_id, ctx),
            documentation_popup: Popup::new(widget_id, ctx),
        }
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.result_list.is_animating()
            || self.detail_popup.is_animating(ctx)
            || self.documentation_popup.is_animating(ctx)
    }

    pub fn receive_msgs(
        &mut self,
        doc: Option<&mut Doc>,
        ctx: &mut Ctx,
    ) -> Option<CompletionListResult> {
        let result_input = self.result_list.receive_msgs(ctx);

        let mut completion_result = None;

        match result_input {
            ResultListInput::Complete
            | ResultListInput::Submit {
                kind: ResultListSubmitKind::Normal,
            } => {
                if let Some(doc) = doc {
                    completion_result = self.perform_result_action(doc, ctx);
                }

                self.hide(ctx);
            }
            ResultListInput::Close => self.hide(ctx),
            ResultListInput::FocusChanged => self.set_popups_shown(ctx),
            _ => {}
        }

        self.detail_popup.receive_msgs(ctx);
        self.documentation_popup.receive_msgs(ctx);

        completion_result
    }

    pub fn update(&mut self, tab: &Tab, doc: &mut Doc, ctx: &mut Ctx, dt: f32) {
        if self.needs_results {
            self.needs_results = false;
            self.update_results(doc, ctx);
        }

        if let Some(CompletionResult::Completion {
            item,
            resolve_state: resolve_state @ CompletionResolveState::NeedsRequest,
        }) = self.result_list.get_focused_mut()
        {
            *resolve_state = CompletionResolveState::NeedsResponse;

            if let Some(sent_request) = Self::lsp_completion_item_resolve(item, doc, ctx) {
                let index = self.result_list.focused_index();

                self.lsp_expected_responses.insert(sent_request.id, index);
            }
        }

        self.update_popups(tab, doc, ctx, dt);
    }

    fn set_popups_shown(&mut self, ctx: &mut Ctx) {
        let Some(CompletionResult::Completion {
            item,
            resolve_state,
        }) = self.result_list.get_focused()
        else {
            self.detail_popup.hide(ctx.ui);
            self.documentation_popup.hide(ctx.ui);
            return;
        };

        if *resolve_state != CompletionResolveState::Resolved {
            return;
        }

        if let Some(detail) = &item.detail {
            self.detail_popup.show(detail, "", ctx);
        } else {
            self.detail_popup.hide(ctx.ui);
        }

        if let Some(documentation) = &item.documentation {
            self.documentation_popup
                .show(documentation.text(), documentation.extension(), ctx);
        } else {
            self.documentation_popup.hide(ctx.ui);
        }
    }

    fn update_popups(&mut self, tab: &Tab, doc: &Doc, ctx: &mut Ctx, dt: f32) {
        if !ctx.ui.is_visible(self.result_list.widget_id()) {
            return;
        }

        let position = doc.cursor(CursorIndex::Main).position;

        let visual_position = doc
            .position_to_visual(position, tab.camera.position().floor(), ctx.gfx)
            .offset_by(tab.doc_bounds(ctx.ui));

        ctx.ui.set_popup(
            self.widget_id,
            Some(Rect::new(visual_position.x, visual_position.y, 0.0, 0.0)),
        );

        self.result_list.update(ctx, dt, |result| result.label());

        let gfx = &ctx.gfx;

        let width = (self.result_list.longest_result_length() as f32 + 2.0) * gfx.glyph_width();
        let position = ctx.ui.bounds(self.widget_id).position();

        let result_list_bounds = Rect::new(
            position.x - (self.prefix.len() as f32 + 1.0) * gfx.glyph_width() + gfx.border_width(),
            position.y + gfx.line_height(),
            width,
            self.result_list
                .desired_height(Self::MAX_VISIBLE_RESULTS, gfx),
        );

        ctx.ui
            .set_popup(self.result_list.widget_id(), Some(result_list_bounds));

        let mut position = VisualPosition::new(
            result_list_bounds.right(),
            result_list_bounds.y + gfx.border_width(),
        );

        self.detail_popup
            .update(position, PopupAlignment::TopLeft, ctx, dt);

        if ctx.ui.is_visible(self.detail_popup.widget_id()) {
            let detail_popup_bounds = ctx.ui.bounds(self.detail_popup.widget_id());
            position.y += detail_popup_bounds.height - ctx.gfx.border_width();
        }

        self.documentation_popup
            .update(position, PopupAlignment::TopLeft, ctx, dt);
    }

    fn show_results(&mut self, ctx: &mut Ctx) {
        if self.result_list.is_empty() {
            self.hide(ctx);
            return;
        }

        let result_list_id = self.result_list.widget_id();
        ctx.ui.show(result_list_id);
        ctx.ui.focus(result_list_id);

        self.set_popups_shown(ctx);
    }

    fn lsp_completion_item_resolve(
        item: &DecodedCompletionItem,
        doc: &Doc,
        ctx: &mut Ctx,
    ) -> Option<LspSentRequest> {
        let language_server = doc.get_language_server_mut(ctx)?;

        Some(language_server.completion_item_resolve(item.clone(), doc))
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        if !ctx.ui.is_visible(self.widget_id) {
            return;
        }

        self.result_list
            .draw(ctx, |result, theme| (result.label(), theme.normal));

        let theme = &ctx.config.theme;

        self.detail_popup.draw(Some(theme.subtle), ctx);
        self.documentation_popup.draw(None, ctx);
    }

    pub fn lsp_resolve_completion_item(
        &mut self,
        id: Option<usize>,
        item: DecodedCompletionItem,
        ctx: &mut Ctx,
    ) {
        let Some(id) = id else {
            return;
        };

        let Entry::Occupied(index) = self.lsp_expected_responses.entry(id) else {
            return;
        };

        let index = index.remove();

        let Some(CompletionResult::Completion {
            item: existing_item,
            resolve_state,
        }) = &mut self.result_list.get_mut(index)
        else {
            return;
        };

        *existing_item = item;
        *resolve_state = CompletionResolveState::Resolved;

        self.set_popups_shown(ctx);
    }

    pub fn lsp_update_completion_results(
        &mut self,
        mut items: Vec<DecodedCompletionItem>,
        needs_resolve: bool,
        doc: &Doc,
        ctx: &mut Ctx,
    ) {
        self.clear_results();

        let resolve_state = if needs_resolve {
            CompletionResolveState::NeedsRequest
        } else {
            CompletionResolveState::Resolved
        };

        if items.is_empty() {
            self.add_token_results(doc, ctx);
        }

        items.sort_by(|a, b| a.sort_text().cmp(b.sort_text()));

        items.sort_by(|a, b| {
            let a = a.filter_text();
            let b = b.filter_text();

            let a_score = score_fuzzy_match(a, &self.prefix);
            let b_score = score_fuzzy_match(b, &self.prefix);

            b_score.total_cmp(&a_score).then(a.len().cmp(&b.len()))
        });

        for item in items {
            self.result_list.push(CompletionResult::Completion {
                item,
                resolve_state,
            });
        }

        self.show_results(ctx);
    }

    pub fn lsp_update_code_action_results(
        &mut self,
        results: Vec<DecodedCodeActionResult>,
        ctx: &mut Ctx,
    ) {
        self.hide(ctx);

        for result in results {
            match result {
                DecodedCodeActionResult::Command(command) => {
                    self.result_list.push(CompletionResult::Command(command));
                }
                DecodedCodeActionResult::CodeAction(code_action) => {
                    let index = if code_action.is_preferred {
                        0
                    } else {
                        self.result_list.len()
                    };

                    self.result_list
                        .insert(index, CompletionResult::CodeAction(code_action));
                }
            }
        }

        self.show_results(ctx);
    }

    pub fn show(&mut self, parent_id: WidgetId, ctx: &mut Ctx) {
        ctx.ui.reparent_widget(self.widget_id, parent_id);

        self.needs_results = true;
    }

    fn update_results(&mut self, doc: &mut Doc, ctx: &mut Ctx) {
        self.prefix.clear();

        let Some(prefix) = doc.get_completion_prefix(ctx.gfx) else {
            self.hide(ctx);

            return;
        };

        self.prefix.push_str(prefix);

        if doc.get_language_server_mut(ctx).is_some() {
            doc.lsp_completion(ctx);

            return;
        }

        self.hide(ctx);
        self.add_token_results(doc, ctx);
    }

    pub fn hide(&mut self, ctx: &mut Ctx) {
        ctx.ui.hide(self.result_list.widget_id());
        ctx.ui.hide(self.detail_popup.widget_id());
        ctx.ui.hide(self.documentation_popup.widget_id());

        self.clear_results();
    }

    fn add_token_results(&mut self, doc: &Doc, ctx: &mut Ctx) {
        if self.prefix.is_empty() {
            return;
        }

        doc.tokens().traverse(&self.prefix, |result| {
            self.result_list
                .push(CompletionResult::SimpleCompletion(result));
        });

        self.show_results(ctx);
    }

    fn clear_results(&mut self) {
        self.needs_results = false;
        self.result_list.drain();
        self.lsp_expected_responses.clear();
    }

    fn perform_result_action(
        &mut self,
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) -> Option<CompletionListResult> {
        let result = self.result_list.remove()?;

        match result {
            CompletionResult::SimpleCompletion(text) => {
                self.complete_at_cursors(&text, doc, ctx);

                None
            }
            CompletionResult::Completion { mut item, .. } => {
                self.perform_completion_item(&item, doc, ctx);
                doc.lsp_apply_edit_list(&mut item.additional_text_edits, ctx);

                None
            }
            CompletionResult::Command(command) => Some(CompletionListResult {
                command: Some(command),
                ..Default::default()
            }),
            CompletionResult::CodeAction(code_action) => Some(CompletionListResult {
                edit_lists: code_action.edit_lists,
                command: code_action.command,
            }),
        }
    }

    fn perform_completion_item(&self, item: &DecodedCompletionItem, doc: &mut Doc, ctx: &mut Ctx) {
        let insert_text = item.insert_text();

        let Some(DecodedRange { start, end }) = item.range() else {
            self.complete_at_cursors(insert_text, doc, ctx);
            return;
        };

        let main_position = doc.cursor(CursorIndex::Main).position;

        // According the LSP, start and end should always be on the
        // requested line, but just in case...
        if start.y != main_position.y || end.y != main_position.y {
            doc.delete(start, end, ctx);
            doc.insert(start, insert_text, ctx);
            return;
        }

        for index in doc.cursor_indices() {
            let position = doc.cursor(index).position;

            let start_x = start.x - main_position.x + position.x;
            let end_x = end.x - main_position.x + position.x;

            let start = Position::new(start_x, position.y);
            let end = Position::new(end_x, position.y);

            doc.delete(start, end, ctx);
            doc.insert(start, insert_text, ctx);
        }
    }

    fn complete_at_cursors(&self, text: &str, doc: &mut Doc, ctx: &mut Ctx) {
        for index in doc.cursor_indices() {
            let cursor = doc.cursor(index);

            if cursor.position.x < self.prefix.len() {
                continue;
            }

            let Some(line) = doc.get_line(cursor.position.y) else {
                continue;
            };

            if !line[..cursor.position.x].ends_with(&self.prefix) {
                continue;
            }

            let end = cursor.position;
            let start = Position::new(end.x - self.prefix.len(), end.y);

            doc.delete(start, end, ctx);
        }

        doc.insert_at_cursors(text, ctx);
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }
}
