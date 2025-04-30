use std::path::PathBuf;

use serde_json::value::RawValue;

use crate::{
    ctx::Ctx,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    input::action::action_keybind,
    lsp::types::{CodeAction, CodeActionResult, Command, CompletionItem},
    platform::gfx::Gfx,
    text::{cursor_index::CursorIndex, doc::Doc, grapheme, line_pool::LinePool},
    ui::{
        core::{Ui, Widget},
        result_list::{ResultList, ResultListInput, ResultListSubmitKind},
    },
};

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

#[derive(Debug)]
pub struct CompletionCommand {
    pub command: String,
    pub arguments: Vec<Box<RawValue>>,
}

#[derive(Debug)]
pub struct CompletionTextEdit {
    pub range: (Position, Position),
    pub new_text: String,
}

enum CompletionResultAction {
    Completion {
        insert_text: Option<String>,
        range: Option<(Position, Position)>,
    },
    Command(CompletionCommand),
    CodeAction {
        edit: Vec<(String, Vec<CompletionTextEdit>)>,
        command: Option<CompletionCommand>,
    },
}

struct CompletionResult {
    label: String,
    action: CompletionResultAction,
}

impl CompletionResult {
    fn push_to_pool(self, pool: &mut LinePool) {
        pool.push(self.label);

        match self.action {
            CompletionResultAction::Completion { insert_text, .. } => {
                if let Some(insert_text) = insert_text {
                    pool.push(insert_text);
                }
            }
            CompletionResultAction::Command(command) => pool.push(command.command),
            CompletionResultAction::CodeAction { edit, command } => {
                for (uri, edits) in edit {
                    pool.push(uri);

                    for edit in edits {
                        pool.push(edit.new_text);
                    }
                }

                if let Some(command) = command {
                    pool.push(command.command);
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct CompletionListResult {
    pub edit: Vec<(String, Vec<CompletionTextEdit>)>,
    pub command: Option<CompletionCommand>,
}

pub struct CompletionList {
    completion_result_list: ResultList<CompletionResult>,
    completion_result_pool: LinePool,
    completion_prefix: String,

    lsp_completion_state: LspCompletionState,
    lsp_pending_doc_path: PathBuf,
    lsp_are_pending_results_valid: bool,

    should_open: bool,
}

impl CompletionList {
    pub fn new() -> Self {
        Self {
            completion_result_list: ResultList::new(MAX_VISIBLE_COMPLETION_RESULTS),
            completion_result_pool: LinePool::new(),
            completion_prefix: String::new(),

            lsp_completion_state: LspCompletionState::Idle,
            lsp_pending_doc_path: PathBuf::new(),
            lsp_are_pending_results_valid: false,

            should_open: false,
        }
    }

    pub fn is_animating(&self) -> bool {
        self.completion_result_list.is_animating()
    }

    pub fn layout(&mut self, visual_position: VisualPosition, gfx: &mut Gfx) {
        let min_y = self.completion_result_list.min_visible_result_index();
        let max_y =
            (min_y + MAX_VISIBLE_COMPLETION_RESULTS).min(self.completion_result_list.results.len());
        let mut longest_visible_result = 0;

        for y in min_y..max_y {
            longest_visible_result =
                longest_visible_result.max(self.completion_result_list.results[y].label.len());
        }

        self.completion_result_list.layout(
            Rect::new(
                visual_position.x - (self.completion_prefix.len() as f32 + 1.0) * gfx.glyph_width()
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
        let are_results_focused = !self.completion_result_list.results.is_empty();

        let result_input = self.completion_result_list.update(
            widget,
            ui,
            ctx.window,
            is_visible,
            are_results_focused,
        );

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
        self.completion_result_list.update_camera(dt);
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        self.completion_result_list
            .draw(ctx, |result| &result.label);
    }

    fn lsp_add_completion_results(&mut self, mut items: Vec<CompletionItem>) {
        items.retain(|item| item.filter_text().starts_with(&self.completion_prefix));
        items.sort_by(|a, b| a.sort_text().cmp(b.sort_text()));

        for item in items {
            let (label, insert_text, range) = if let Some(text_edit) = &item.text_edit {
                (
                    item.label,
                    Some(text_edit.new_text.clone()),
                    Some(text_edit.range),
                )
            } else {
                (item.label, item.insert_text, None)
            };

            let mut label_string = self.completion_result_pool.pop();
            label_string.push_str(label);

            let insert_text_string = insert_text.map(|insert_text| {
                let mut insert_text_string = self.completion_result_pool.pop();
                insert_text_string.push_str(&insert_text);
                insert_text_string
            });

            self.completion_result_list.results.push(CompletionResult {
                label: label_string,
                action: CompletionResultAction::Completion {
                    insert_text: insert_text_string,
                    range,
                },
            });
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
                    let mut label = self.completion_result_pool.pop();
                    label.push_str(command.title);

                    let mut command_string = self.completion_result_pool.pop();
                    command_string.push_str(command.command);

                    self.completion_result_list.results.push(CompletionResult {
                        label,
                        action: CompletionResultAction::Command(CompletionCommand {
                            command: command_string,
                            arguments: command.arguments,
                        }),
                    });
                }
                CodeActionResult::CodeAction(code_action) => {
                    let mut label = self.completion_result_pool.pop();
                    label.push_str(code_action.title);

                    let command = if let Some(command) = code_action.command {
                        let mut command_string = self.completion_result_pool.pop();
                        command_string.push_str(command.command);

                        Some(CompletionCommand {
                            command: command_string,
                            arguments: command.arguments,
                        })
                    } else {
                        None
                    };

                    let mut completion_edit = Vec::new();

                    for (uri, edits) in code_action.edit.into_iter() {
                        let edits = edits
                            .into_iter()
                            .map(|edit| {
                                let mut new_text = self.completion_result_pool.pop();
                                new_text.push_str(&edit.new_text);

                                CompletionTextEdit {
                                    range: edit.range,
                                    new_text,
                                }
                            })
                            .collect();

                        let mut uri_string = self.completion_result_pool.pop();
                        uri_string.push_str(uri);

                        completion_edit.push((uri_string, edits));
                    }

                    self.completion_result_list.results.push(CompletionResult {
                        label,
                        action: CompletionResultAction::CodeAction {
                            edit: completion_edit,
                            command,
                        },
                    });
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
            self.completion_prefix.clear();
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

        self.completion_prefix.push_str(prefix);

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

        if !self.completion_prefix.is_empty() {
            doc.tokens().traverse(
                &self.completion_prefix,
                &mut self.completion_result_pool,
                |label| {
                    self.completion_result_list.results.push(CompletionResult {
                        label,
                        action: CompletionResultAction::Completion {
                            insert_text: None,
                            range: None,
                        },
                    });
                },
            );
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
        for result in self.completion_result_list.drain() {
            result.push_to_pool(&mut self.completion_result_pool);
        }
    }

    fn perform_result_action(
        &mut self,
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) -> Option<CompletionListResult> {
        let result = self.completion_result_list.remove_selected_result()?;

        match result.action {
            CompletionResultAction::Completion { insert_text, range } => {
                let insert_text = insert_text.as_ref().unwrap_or(&result.label);

                if let Some((start, end)) = range {
                    doc.delete(start, end, ctx);
                    doc.insert(start, insert_text, ctx);
                } else {
                    doc.insert_at_cursors(&insert_text[self.completion_prefix.len()..], ctx);
                }

                None
            }
            CompletionResultAction::Command(command) => Some(CompletionListResult {
                command: Some(command),
                ..Default::default()
            }),
            CompletionResultAction::CodeAction { edit, command } => {
                println!("performing code action");
                Some(CompletionListResult { edit, command })
            }
        }
    }

    pub fn bounds(&self) -> Rect {
        self.completion_result_list.bounds()
    }
}
