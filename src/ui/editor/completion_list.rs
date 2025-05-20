use std::collections::{hash_map::Entry, HashMap};

use crate::{
    ctx::Ctx,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    input::action::action_keybind,
    lsp::{
        types::{
            Command, DecodedCodeAction, DecodedCodeActionResult, DecodedCompletionItem,
            DecodedEditList, DecodedRange,
        },
        LspSentRequest,
    },
    pool::Pooled,
    text::{cursor_index::CursorIndex, doc::Doc},
    ui::{
        core::{Ui, WidgetId, WidgetSettings},
        popup::{Popup, PopupAlignment},
        result_list::{ResultList, ResultListInput, ResultListSubmitKind},
    },
};

const MAX_VISIBLE_COMPLETION_RESULTS: usize = 10;

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
            CompletionResult::SimpleCompletion(text) => text,
            CompletionResult::Completion { item, .. } => &item.label,
            CompletionResult::Command(command) => &command.title,
            CompletionResult::CodeAction(code_action) => &code_action.title,
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

    result_list: ResultList<CompletionResult>,
    // Prevents the result list from shrinking as it's being scrolled through.
    min_width: f32,
    prefix: String,

    should_open: bool,

    lsp_expected_responses: HashMap<usize, usize>,

    detail_popup: Popup,
    documentation_popup: Popup,
}

impl CompletionList {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        let widget_id = ui.new_widget(
            parent_id,
            WidgetSettings {
                is_component: true,
                ..Default::default()
            },
        );

        Self {
            widget_id,

            result_list: ResultList::new(MAX_VISIBLE_COMPLETION_RESULTS, false, widget_id, ui),
            min_width: 0.0,
            prefix: String::new(),

            should_open: false,

            lsp_expected_responses: HashMap::new(),

            detail_popup: Popup::new(widget_id, ui),
            documentation_popup: Popup::new(widget_id, ui),
        }
    }

    pub fn is_animating(&self) -> bool {
        self.result_list.is_animating()
            || self.detail_popup.is_animating()
            || self.documentation_popup.is_animating()
    }

    pub fn layout(&mut self, visual_position: VisualPosition, ctx: &mut Ctx) {
        let min_y = self.result_list.min_visible_result_index();
        let max_y = (min_y + MAX_VISIBLE_COMPLETION_RESULTS).min(self.result_list.len());
        let mut longest_visible_result = 0;

        for y in min_y..max_y {
            let Some(result) = self.result_list.get(y) else {
                continue;
            };

            let label = result.label();

            longest_visible_result = longest_visible_result.max(label.len());
        }

        let gfx = &ctx.gfx;

        let width = (longest_visible_result as f32 + 2.0) * gfx.glyph_width();
        let width = width.max(self.min_width);

        self.min_width = width;

        self.result_list.layout(
            Rect::new(
                visual_position.x - (self.prefix.len() as f32 + 1.0) * gfx.glyph_width()
                    + gfx.border_width(),
                visual_position.y + gfx.line_height(),
                width,
                0.0,
            ),
            ctx.ui,
            gfx,
        );

        let result_list_bounds = ctx.ui.widget(self.result_list.widget_id()).bounds;

        let mut position = VisualPosition::new(
            result_list_bounds.right() - gfx.border_width(),
            result_list_bounds.y,
        );

        self.detail_popup
            .layout(position, PopupAlignment::TopLeft, ctx);

        if ctx.ui.is_visible(self.detail_popup.widget_id()) {
            position.y +=
                ctx.ui.widget(self.detail_popup.widget_id()).bounds.height - ctx.gfx.border_width();
        }

        self.documentation_popup
            .layout(position, PopupAlignment::TopLeft, ctx);
    }

    pub fn update(&mut self, doc: &mut Doc, ctx: &mut Ctx) -> Option<CompletionListResult> {
        let result_input = self.result_list.update(ctx);

        let mut completion_result = None;

        match result_input {
            ResultListInput::Complete
            | ResultListInput::Submit {
                kind: ResultListSubmitKind::Normal,
            } => {
                completion_result = self.perform_result_action(doc, ctx);
                self.clear(ctx);
            }
            ResultListInput::Close => self.clear(ctx),
            _ => {}
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

        self.set_popups_shown(ctx);
        self.detail_popup.update(ctx);
        self.documentation_popup.update(ctx);

        self.should_open = self.should_open(ctx);

        completion_result
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
            self.detail_popup.show(detail, ctx);
        } else {
            self.detail_popup.hide(ctx.ui);
        }

        if let Some(documentation) = &item.documentation {
            self.documentation_popup.show(documentation.text(), ctx);
        } else {
            self.documentation_popup.hide(ctx.ui);
        }
    }

    pub fn update_camera(&mut self, ctx: &mut Ctx, dt: f32) {
        self.result_list.update_camera(ctx.ui, dt);

        self.detail_popup.update_camera(ctx, dt);
        self.documentation_popup.update_camera(ctx, dt);
    }

    fn lsp_completion_item_resolve(
        item: &DecodedCompletionItem,
        doc: &Doc,
        ctx: &mut Ctx,
    ) -> Option<LspSentRequest> {
        let language_server = doc.get_language_server_mut(ctx)?;

        Some(language_server.completion_item_resolve(item.clone(), doc))
    }

    fn should_open(&self, ctx: &mut Ctx) -> bool {
        let mut grapheme_handler = ctx.ui.grapheme_handler(self.widget_id(), ctx.window);

        if grapheme_handler.next(ctx.window).is_some() {
            grapheme_handler.unprocessed(ctx.window);
            return true;
        }

        let mut keybind_handler = ctx.ui.keybind_handler(self.widget_id(), ctx.window);

        while let Some(action) = keybind_handler.next_action(ctx) {
            keybind_handler.unprocessed(ctx.window, action.keybind);

            if matches!(action, action_keybind!(key: Backspace)) {
                return true;
            }
        }

        false
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        self.result_list
            .draw(ctx, |result, theme| (result.label(), theme.normal));

        let theme = &ctx.config.theme;

        self.detail_popup.draw(theme.subtle, ctx);
        self.documentation_popup.draw(theme.normal, ctx);
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
        ctx: &mut Ctx,
    ) {
        self.clear(ctx);

        items.retain(|item| item.filter_text().starts_with(&self.prefix));
        items.sort_by(|a, b| a.sort_text().cmp(b.sort_text()));

        let resolve_state = if needs_resolve {
            CompletionResolveState::NeedsRequest
        } else {
            CompletionResolveState::Resolved
        };

        for item in items {
            self.result_list.push(CompletionResult::Completion {
                item,
                resolve_state,
            });
        }
    }

    pub fn lsp_update_code_action_results(
        &mut self,
        results: Vec<DecodedCodeActionResult>,
        ctx: &mut Ctx,
    ) {
        self.clear(ctx);

        for result in results {
            match result {
                DecodedCodeActionResult::Command(command) => {
                    self.result_list
                        .results
                        .push(CompletionResult::Command(command));
                }
                DecodedCodeActionResult::CodeAction(code_action) => {
                    let index = if code_action.is_preferred {
                        0
                    } else {
                        self.result_list.len()
                    };

                    self.result_list
                        .results
                        .insert(index, CompletionResult::CodeAction(code_action));
                }
            }
        }
    }

    pub fn update_results(
        &mut self,
        did_cursor_move: bool,
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) -> Option<()> {
        if !self.should_open {
            if did_cursor_move {
                self.prefix.clear();
                self.clear(ctx);
            }

            return None;
        }

        self.prefix.clear();

        let Some(prefix) = doc.get_completion_prefix(ctx.gfx) else {
            self.clear(ctx);

            return None;
        };

        self.prefix.push_str(prefix);

        if doc.get_language_server_mut(ctx).is_some() {
            doc.lsp_completion(ctx);

            return Some(());
        }

        self.clear(ctx);

        if !self.prefix.is_empty() {
            doc.tokens().traverse(&self.prefix, |result| {
                self.result_list
                    .results
                    .push(CompletionResult::SimpleCompletion(result));
            });
        }

        Some(())
    }

    pub fn clear(&mut self, ctx: &mut Ctx) {
        self.result_list.drain();
        self.lsp_expected_responses.clear();

        self.set_popups_shown(ctx);

        self.min_width = 0.0;
    }

    fn perform_result_action(
        &mut self,
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) -> Option<CompletionListResult> {
        let result = self.result_list.remove()?;

        match result {
            CompletionResult::SimpleCompletion(text) => {
                doc.insert_at_cursors(&text[self.prefix.len()..], ctx);

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
            doc.insert_at_cursors(&insert_text[self.prefix.len()..], ctx);
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

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }
}
