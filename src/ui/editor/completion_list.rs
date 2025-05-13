use std::{
    collections::{hash_map::Entry, HashMap},
    path::PathBuf,
};

use crate::{
    ctx::Ctx,
    geometry::{position::Position, rect::Rect, visual_position::VisualPosition},
    input::action::action_keybind,
    lsp::{
        types::{
            Command, DecodedCodeAction, DecodedCodeActionResult, DecodedCompletionItem,
            DecodedEditList, DecodedRange, Documentation,
        },
        LspSentRequest,
    },
    platform::gfx::Gfx,
    pool::Pooled,
    text::{cursor_index::CursorIndex, doc::Doc},
    ui::{
        core::{Ui, Widget},
        popup::{draw_popup, PopupAlignment},
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

// Used to keep a previous completion item's popup open while the next one is loading.
#[derive(Debug)]
enum CompletionPopupCache {
    PreviousIndex(usize),
    PreviousItem {
        detail: Option<Pooled<String>>,
        documentation: Option<Documentation>,
    },
    None,
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
    handled_path: PathBuf,
    has_handled_path: bool,

    lsp_expected_responses: HashMap<usize, usize>,
    popup_cache: CompletionPopupCache,
}

impl CompletionList {
    pub fn new() -> Self {
        Self {
            result_list: ResultList::new(MAX_VISIBLE_COMPLETION_RESULTS),
            min_width: 0.0,
            prefix: String::new(),

            should_open: false,
            handled_path: PathBuf::new(),
            has_handled_path: false,

            lsp_expected_responses: HashMap::new(),
            popup_cache: CompletionPopupCache::None,
        }
    }

    pub fn is_animating(&self) -> bool {
        self.result_list.is_animating()
    }

    pub fn layout(&mut self, visual_position: VisualPosition, gfx: &mut Gfx) {
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
        let are_results_focused = !self.result_list.is_empty();

        if matches!(self.popup_cache, CompletionPopupCache::None) {
            self.popup_cache = CompletionPopupCache::PreviousIndex(self.result_list.focused_index())
        }

        let result_input =
            self.result_list
                .update(widget, ui, ctx.window, is_visible, are_results_focused);

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

        self.should_open = self.get_should_open(ui, widget, ctx);

        completion_result
    }

    fn lsp_completion_item_resolve(
        item: &DecodedCompletionItem,
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) -> Option<LspSentRequest> {
        let language_server = doc.get_language_server_mut(ctx)?;

        Some(language_server.completion_item_resolve(item.clone(), doc))
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

        false
    }

    pub fn update_camera(&mut self, dt: f32) {
        self.result_list.update_camera(dt);
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        self.result_list
            .draw(ctx, |result, theme| (result.label(), theme.normal));

        let Some(focused_result) = self.result_list.get_focused() else {
            return;
        };

        let CompletionResult::Completion {
            item:
                DecodedCompletionItem {
                    ref detail,
                    ref documentation,
                    ..
                },
            resolve_state,
        } = &focused_result
        else {
            return;
        };

        if *resolve_state == CompletionResolveState::Resolved {
            self.popup_cache = CompletionPopupCache::None;
        }

        let (detail, documentation) = match &self.popup_cache {
            CompletionPopupCache::PreviousIndex(index) => {
                if let Some(CompletionResult::Completion {
                    item:
                        DecodedCompletionItem {
                            detail,
                            documentation,
                            ..
                        },
                    ..
                }) = self.result_list.get(*index)
                {
                    (detail, documentation)
                } else {
                    (detail, documentation)
                }
            }
            CompletionPopupCache::PreviousItem {
                detail,
                documentation,
            } => (detail, documentation),
            _ => (detail, documentation),
        };

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        let mut position = VisualPosition::new(
            self.result_list.bounds().right() - gfx.border_width(),
            self.result_list.bounds().y,
        );

        if let Some(detail) = detail {
            let detail_bounds = draw_popup(
                detail,
                position,
                PopupAlignment::TopLeft,
                theme.subtle,
                theme,
                gfx,
            );

            position.y += detail_bounds.height - gfx.border_width();
        }

        if let Some(documentation) = documentation {
            draw_popup(
                documentation.text(),
                position,
                PopupAlignment::TopLeft,
                theme.normal,
                theme,
                gfx,
            );
        }
    }

    pub fn lsp_resolve_completion_item(&mut self, id: Option<usize>, item: DecodedCompletionItem) {
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
    }

    pub fn lsp_update_completion_results(
        &mut self,
        mut items: Vec<DecodedCompletionItem>,
        needs_resolve: bool,
    ) {
        if let Some(CompletionResult::Completion {
            item:
                DecodedCompletionItem {
                    detail,
                    documentation,
                    ..
                },
            ..
        }) = self.result_list.remove()
        {
            self.popup_cache = CompletionPopupCache::PreviousItem {
                detail,
                documentation,
            };
        }

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
        doc: &mut Doc,
        handled_position: Option<Position>,
        ctx: &mut Ctx,
    ) -> Option<()> {
        let position = doc.get_cursor(CursorIndex::Main).position;
        let is_position_different = Some(position) != handled_position;
        let is_path_different =
            self.has_handled_path.then_some(self.handled_path.as_path()) != doc.path().some_path();

        self.handled_path.clear();
        self.has_handled_path = false;

        if let Some(path) = doc.path().some() {
            self.handled_path.push(path);
            self.has_handled_path = true;
        }

        if !self.should_open {
            if is_position_different || is_path_different {
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
                let insert_text = item.insert_text();

                if let Some(DecodedRange { start, end }) = item.range() {
                    doc.delete(start, end, ctx);
                    doc.insert(start, insert_text, ctx);
                } else {
                    doc.insert_at_cursors(&insert_text[self.prefix.len()..], ctx);
                }

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

    pub fn bounds(&self) -> Rect {
        self.result_list.bounds()
    }
}
