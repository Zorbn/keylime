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
    platform::gfx::Gfx,
    pool::Pooled,
    text::{cursor_index::CursorIndex, doc::Doc},
    ui::{
        core::{Ui, WidgetId},
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
        Self {
            result_list: ResultList::new(MAX_VISIBLE_COMPLETION_RESULTS, parent_id, ui),
            min_width: 0.0,
            prefix: String::new(),

            should_open: false,

            lsp_expected_responses: HashMap::new(),

            detail_popup: Popup::new(parent_id, ui),
            documentation_popup: Popup::new(parent_id, ui),
        }
    }

    pub fn is_animating(&self) -> bool {
        self.result_list.is_animating()
    }

    pub fn layout(&mut self, visual_position: VisualPosition, ui: &mut Ui, gfx: &mut Gfx) {
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
            ui,
            gfx,
        );

        let result_list_bounds = ui.widget(self.result_list.widget_id()).bounds;

        let mut position = VisualPosition::new(
            result_list_bounds.right() - gfx.border_width(),
            result_list_bounds.y,
        );

        self.detail_popup
            .layout(position, PopupAlignment::TopLeft, ui, gfx);

        if ui.is_visible(self.detail_popup.widget_id()) {
            position.y +=
                ui.widget(self.detail_popup.widget_id()).bounds.height - gfx.border_width();
        }

        self.documentation_popup
            .layout(position, PopupAlignment::TopLeft, ui, gfx);
    }

    pub fn update(
        &mut self,
        ui: &mut Ui,
        doc: &mut Doc,
        is_visible: bool,
        ctx: &mut Ctx,
    ) -> Option<CompletionListResult> {
        let are_results_focused = !self.result_list.is_empty();

        let result_input = self
            .result_list
            .update(ui, ctx.window, is_visible, are_results_focused);

        let mut completion_result = None;

        match result_input {
            ResultListInput::Complete
            | ResultListInput::Submit {
                kind: ResultListSubmitKind::Normal,
            } => {
                completion_result = self.perform_result_action(doc, ctx);
                self.clear();
            }
            ResultListInput::Close => self.clear(),
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

        self.update_popups(ui);

        self.should_open = self.should_open(ui, ctx);

        completion_result
    }

    fn update_popups(&mut self, ui: &mut Ui) {
        let Some(CompletionResult::Completion {
            item,
            resolve_state,
        }) = self.result_list.get_focused()
        else {
            self.detail_popup.hide(ui);
            self.documentation_popup.hide(ui);
            return;
        };

        if *resolve_state != CompletionResolveState::Resolved {
            return;
        }

        self.detail_popup.hide(ui);
        self.documentation_popup.hide(ui);

        if let Some(detail) = &item.detail {
            self.detail_popup.show(detail, ui);
        }

        if let Some(documentation) = &item.documentation {
            self.detail_popup.show(documentation.text(), ui);
        }
    }

    fn lsp_completion_item_resolve(
        item: &DecodedCompletionItem,
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) -> Option<LspSentRequest> {
        let language_server = doc.get_language_server_mut(ctx)?;

        Some(language_server.completion_item_resolve(item.clone(), doc))
    }

    fn should_open(&mut self, ui: &Ui, ctx: &mut Ctx) -> bool {
        let widget_id = self.result_list.widget_id();
        let mut grapheme_handler = ui.grapheme_handler(widget_id, ctx.window);

        if grapheme_handler.next(ctx.window).is_some() {
            grapheme_handler.unprocessed(ctx.window);
            return true;
        }

        let mut action_handler = ui.action_handler(widget_id, ctx.window);

        while let Some(action) = action_handler.next(ctx.window) {
            action_handler.unprocessed(ctx.window, action);

            if matches!(action, action_keybind!(key: Backspace)) {
                return true;
            }
        }

        false
    }

    pub fn update_camera(&mut self, ui: &Ui, dt: f32) {
        self.result_list.update_camera(ui, dt);
    }

    pub fn draw(&mut self, ui: &Ui, ctx: &mut Ctx) {
        self.result_list
            .draw(ui, ctx, |result, theme| (result.label(), theme.normal));

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        self.detail_popup.draw(theme.subtle, theme, ui, gfx);
        self.documentation_popup.draw(theme.normal, theme, ui, gfx);
    }

    pub fn lsp_resolve_completion_item(
        &mut self,
        id: Option<usize>,
        item: DecodedCompletionItem,
        ui: &mut Ui,
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

        self.update_popups(ui);
    }

    pub fn lsp_update_completion_results(
        &mut self,
        mut items: Vec<DecodedCompletionItem>,
        needs_resolve: bool,
    ) {
        self.clear();

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

    pub fn lsp_update_code_action_results(&mut self, results: Vec<DecodedCodeActionResult>) {
        self.clear();

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
                self.clear();
            }

            return None;
        }

        self.prefix.clear();

        let Some(prefix) = doc.get_completion_prefix(ctx.gfx) else {
            self.clear();

            return None;
        };

        self.prefix.push_str(prefix);

        if doc.get_language_server_mut(ctx).is_some() {
            doc.lsp_completion(ctx);

            return Some(());
        }

        self.clear();

        if !self.prefix.is_empty() {
            doc.tokens().traverse(&self.prefix, |result| {
                self.result_list
                    .results
                    .push(CompletionResult::SimpleCompletion(result));
            });
        }

        Some(())
    }

    pub fn clear(&mut self) {
        self.result_list.drain();
        self.lsp_expected_responses.clear();

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
}
