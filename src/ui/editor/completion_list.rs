use std::path::PathBuf;

use crate::{
    ctx::Ctx,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    input::action::action_keybind,
    lsp::types::{CodeActionDocumentEdit, CodeActionResult, CompletionItem},
    platform::gfx::Gfx,
    text::{cursor_index::CursorIndex, doc::Doc, grapheme, line_pool::LinePool},
    ui::{
        core::{Ui, Widget},
        result_list::{ResultList, ResultListInput, ResultListSubmitKind},
    },
};

use super::completion_result::{CompletionCommand, CompletionResult, CompletionResultAction};

const MAX_VISIBLE_COMPLETION_RESULTS: usize = 10;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum LspCompletionState {
    Idle,
    Pending,
    MultiplePending,
    ReadyForCompletion,
}

impl LspCompletionState {
    pub fn next(&self) -> LspCompletionState {
        match self {
            LspCompletionState::Idle => LspCompletionState::Pending,
            LspCompletionState::Pending => LspCompletionState::MultiplePending,
            LspCompletionState::MultiplePending => LspCompletionState::MultiplePending,
            _ => *self,
        }
    }
}

#[derive(Debug, Default)]
pub struct CompletionListResult {
    pub edits: Vec<CodeActionDocumentEdit>,
    pub command: Option<CompletionCommand>,
}

pub struct CompletionList {
    result_list: ResultList<CompletionResult>,
    pool: LinePool,
    prefix: String,

    lsp_completion_state: LspCompletionState,
    lsp_pending_doc_path: PathBuf,
    lsp_are_pending_results_valid: bool,

    should_open: bool,
}

impl CompletionList {
    pub fn new() -> Self {
        Self {
            result_list: ResultList::new(MAX_VISIBLE_COMPLETION_RESULTS),
            pool: LinePool::new(),
            prefix: String::new(),

            lsp_completion_state: LspCompletionState::Idle,
            lsp_pending_doc_path: PathBuf::new(),
            lsp_are_pending_results_valid: false,

            should_open: false,
        }
    }

    pub fn is_animating(&self) -> bool {
        self.result_list.is_animating()
    }

    pub fn layout(&mut self, visual_position: VisualPosition, gfx: &mut Gfx) {
        let min_y = self.result_list.min_visible_result_index();
        let max_y = (min_y + MAX_VISIBLE_COMPLETION_RESULTS).min(self.result_list.results.len());
        let mut longest_visible_result = 0;

        for y in min_y..max_y {
            longest_visible_result =
                longest_visible_result.max(self.result_list.results[y].label.len());
        }

        self.result_list.layout(
            Rect::new(
                visual_position.x - (self.prefix.len() as f32 + 1.0) * gfx.glyph_width()
                    + gfx.border_width(),
                visual_position.y + gfx.line_height(),
                (longest_visible_result as f32 + 2.0) * gfx.glyph_width(),
                0.0,
            ),
            gfx,
        );
    }

    pub fn update(
        &mut self,
        ui: &mut Ui,
        widget: &Widget,
        doc: &mut Doc,
        is_visible: bool,
        ctx: &mut Ctx,
    ) -> Option<CompletionListResult> {
        let are_results_focused = !self.result_list.results.is_empty();

        let result_input =
            self.result_list
                .update(widget, ui, ctx.window, is_visible, are_results_focused);

        let mut completion_result = None;

        match result_input {
            ResultListInput::None => {}
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

        self.should_open = self.get_should_open(ui, widget, ctx);

        completion_result
    }

    fn get_should_open(&mut self, ui: &mut Ui, widget: &Widget, ctx: &mut Ctx) -> bool {
        let mut grapheme_handler = ui.get_grapheme_handler(widget, ctx.window);

        if grapheme_handler.next(ctx.window).is_some() {
            grapheme_handler.unprocessed(ctx.window);
            return true;
        }

        let mut action_handler = ui.get_action_handler(widget, ctx.window);

        while let Some(action) = action_handler.next(ctx.window) {
            action_handler.unprocessed(ctx.window, action);

            if matches!(action, action_keybind!(key: Backspace)) {
                return true;
            }
        }

        if self.lsp_completion_state == LspCompletionState::ReadyForCompletion {
            self.lsp_completion_state = LspCompletionState::Idle;
            return true;
        }

        false
    }

    pub fn update_camera(&mut self, dt: f32) {
        self.result_list.update_camera(dt);
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        self.result_list.draw(ctx, |result| &result.label);
    }

    fn lsp_add_completion_results(&mut self, mut items: Vec<CompletionItem>) {
        items.retain(|item| item.filter_text().starts_with(&self.prefix));
        items.sort_by(|a, b| a.sort_text().cmp(b.sort_text()));

        for item in items {
            let result = CompletionResult::from_completion_item(item, &mut self.pool);

            self.result_list.results.push(result);
        }
    }

    pub fn lsp_update_completion_results(&mut self, items: Vec<CompletionItem>) {
        self.clear();

        if self.lsp_are_pending_results_valid {
            self.lsp_add_completion_results(items);
        }

        self.lsp_completion_state =
            if self.lsp_completion_state == LspCompletionState::MultiplePending {
                LspCompletionState::ReadyForCompletion
            } else {
                LspCompletionState::Idle
            };
    }

    fn lsp_add_code_action_results(&mut self, results: Vec<CodeActionResult>) {
        for result in results {
            match result {
                CodeActionResult::Command(command) => {
                    let result = CompletionResult::from_command(command, &mut self.pool);

                    self.result_list.results.push(result);
                }
                CodeActionResult::CodeAction(code_action) => {
                    let (result, is_preferred) =
                        CompletionResult::from_code_action(code_action, &mut self.pool);

                    let index = if is_preferred {
                        0
                    } else {
                        self.result_list.results.len()
                    };

                    self.result_list.results.insert(index, result);
                }
            }
        }
    }

    pub fn lsp_update_code_action_results(&mut self, results: Vec<CodeActionResult>) {
        self.clear();
        self.lsp_add_code_action_results(results);
    }

    // TODO: Simplify all of the clear calls.
    pub fn update_results(
        &mut self,
        doc: &Doc,
        handled_position: Option<Position>,
        ctx: &mut Ctx,
    ) -> Option<()> {
        let position = doc.get_cursor(CursorIndex::Main).position;
        let is_position_different = Some(position) != handled_position;

        if self.lsp_are_pending_results_valid
            && (self.should_open
                || is_position_different
                || Some(self.lsp_pending_doc_path.as_path()) != doc.path().on_drive())
        {
            self.lsp_are_pending_results_valid = false;
        }

        if self.should_open || is_position_different {
            self.prefix.clear();
        }

        if !self.should_open {
            if is_position_different {
                self.clear();
            }

            return None;
        }

        let Some(prefix) = Self::get_completion_prefix(doc, ctx.gfx) else {
            self.clear();

            return None;
        };

        self.prefix.push_str(prefix);

        if doc.get_language_server_mut(ctx).is_some() {
            let path = doc.path().on_drive()?;

            if self.lsp_completion_state == LspCompletionState::Idle {
                doc.lsp_completion(ctx);

                self.lsp_pending_doc_path.clear();
                self.lsp_pending_doc_path.push(path);

                self.lsp_are_pending_results_valid = true;
            }

            self.lsp_completion_state = self.lsp_completion_state.next();

            return Some(());
        }

        self.clear();

        if !self.prefix.is_empty() {
            doc.tokens()
                .traverse(&self.prefix, &mut self.pool, |label| {
                    self.result_list.results.push(CompletionResult {
                        label,
                        action: CompletionResultAction::Completion {
                            insert_text: None,
                            range: None,
                        },
                    });
                });
        }

        Some(())
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

            // These characters aren't included in the completion prefix
            // but they should still trigger a completion.
            if !matches!(grapheme, "." | ":") && prefix_start == prefix_end {
                return None;
            }

            break;
        }

        doc.get_line(prefix_end.y)
            .map(|line| &line[prefix_start.x..prefix_end.x])
    }

    pub fn clear(&mut self) {
        for result in self.result_list.drain() {
            result.push_to_pool(&mut self.pool);
        }
    }

    fn perform_result_action(
        &mut self,
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) -> Option<CompletionListResult> {
        let result = self.result_list.remove_selected_result()?;

        match result.action {
            CompletionResultAction::Completion { insert_text, range } => {
                let insert_text = insert_text.as_ref().unwrap_or(&result.label);

                if let Some((start, end)) = range {
                    doc.delete(start, end, ctx);
                    doc.insert(start, insert_text, ctx);
                } else {
                    doc.insert_at_cursors(&insert_text[self.prefix.len()..], ctx);
                }

                None
            }
            CompletionResultAction::Command(command) => Some(CompletionListResult {
                command: Some(command),
                ..Default::default()
            }),
            CompletionResultAction::CodeAction { edits, command } => {
                Some(CompletionListResult { edits, command })
            }
        }
    }

    pub fn bounds(&self) -> Rect {
        self.result_list.bounds()
    }
}
