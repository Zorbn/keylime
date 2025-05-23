pub mod language_server;
pub mod position_encoding;
pub mod types;
pub mod uri;

use std::{
    collections::{hash_map::Entry, HashMap},
    env::current_dir,
    path::PathBuf,
};

use language_server::{LanguageServer, MessageResult};
use types::{DecodedDiagnostic, DecodedRange, DecodedTextEdit, Message};
use uri::uri_to_path;

use crate::{
    config::Config,
    ctx::Ctx,
    geometry::position::Position,
    platform::process::Process,
    pool::STRING_POOL,
    text::doc::Doc,
    ui::{
        command_palette::{
            find_in_files_mode::FindInFilesMode, references_mode::ReferencesMode,
            rename_mode::RenameMode, CommandPalette,
        },
        editor::Editor,
    },
};

pub struct LspSentRequest {
    pub method: &'static str,
    pub id: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct LspExpectedResponse {
    pub id: usize,
    pub position: Option<Position>,
    pub version: usize,
}

pub struct Lsp {
    servers: HashMap<usize, Option<LanguageServer>>,
    current_dir: Option<PathBuf>,
}

impl Lsp {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            current_dir: current_dir().ok(),
        }
    }

    fn clear(&mut self) {
        self.servers.clear();
    }

    pub fn update_current_dir(&mut self, current_dir: Option<PathBuf>) {
        self.current_dir = current_dir;
        self.clear();
    }

    pub fn update(editor: &mut Editor, command_palette: &mut CommandPalette, ctx: &mut Ctx) {
        while let Some(polled_message) = ctx.lsp.poll() {
            Self::handle_message(polled_message, editor, command_palette, ctx);
        }
    }

    fn handle_message(
        (language_index, message): (usize, Message),
        editor: &mut Editor,
        command_palette: &mut CommandPalette,
        ctx: &mut Ctx,
    ) -> Option<()> {
        let server = ctx.lsp.servers.get_mut(&language_index)?.as_mut()?;
        let encoding = server.position_encoding();

        let (path, method) = server.get_message_path_and_method(&message);
        let method = method.unwrap_or_default();

        let (doc_id, mut doc) = path
            .as_ref()
            .and_then(|path| editor.find_doc_with_id_mut(path))
            .unzip();

        if let Some(ref mut doc) = doc {
            if !doc.lsp_is_response_expected(method, message.id, ctx) {
                return None;
            }
        }

        let server = ctx.lsp.servers.get_mut(&language_index)?.as_mut()?;
        let result = server.handle_message(method, &message)?;

        match result {
            MessageResult::Completion(items) => {
                let doc = doc?;

                // The compiler should perform an in-place collect here because
                // LspCompletionItem and CompletionItem have the same size and alignment.
                let items = items
                    .into_iter()
                    .map(|item| item.decode(encoding, doc))
                    .collect();

                editor.lsp_update_completion_results(
                    items,
                    server.needs_completion_resolve(),
                    doc_id?,
                    ctx,
                );
            }
            MessageResult::CompletionItemResolve(item) => {
                let doc = doc?;
                let item = item.decode(encoding, doc);

                editor
                    .completion_list
                    .lsp_resolve_completion_item(message.id, item, ctx);
            }
            MessageResult::CodeAction(results) => {
                let doc = doc?;
                let results = results
                    .into_iter()
                    .map(|result| result.decode(encoding, doc))
                    .collect();

                editor
                    .completion_list
                    .lsp_update_code_action_results(results, ctx);
            }
            MessageResult::PrepareRename { range, placeholder } => {
                let doc = doc?;
                let DecodedRange { start, end } = range.decode(encoding, doc);

                let placeholder = placeholder.unwrap_or_else(|| {
                    STRING_POOL.init_item(|placeholder| doc.collect_string(start, end, placeholder))
                });

                command_palette.open(Box::new(RenameMode::new(placeholder)), editor, ctx);
            }
            MessageResult::Rename(workspace_edit) => {
                let doc = doc?;
                let edit_lists = workspace_edit.decode(encoding, doc);

                editor.lsp_apply_edit_lists(edit_lists, ctx);
            }
            MessageResult::References(mut results) => {
                results.sort_by(|a, b| a.uri.cmp(b.uri));

                let mut command_palette_results = Vec::new();
                let mut results = results.into_iter().peekable();

                while let Some(result) = results.peek() {
                    let current_uri = result.uri;

                    let Some(path) = uri_to_path(current_uri) else {
                        continue;
                    };

                    editor.with_doc(path, ctx, |doc, ctx| {
                        while let Some(result) = results.peek() {
                            let Some(root) = ctx.lsp.current_dir.as_ref() else {
                                break;
                            };

                            if result.uri != current_uri {
                                break;
                            }

                            let next_result = results.next().unwrap();
                            let result_position = next_result.range.decode(encoding, doc).start;

                            let Some(result) =
                                FindInFilesMode::position_to_result(result_position, root, doc)
                            else {
                                continue;
                            };

                            command_palette_results.push(result);
                        }
                    });
                }

                command_palette.open(
                    Box::new(ReferencesMode::new(command_palette_results)),
                    editor,
                    ctx,
                );
            }
            MessageResult::Definition { path, range } => {
                let (pane, doc_list) = editor.last_focused_pane_and_doc_list_mut(ctx.ui);

                if pane.open_file(&path, doc_list, ctx).is_err() {
                    return None;
                }

                let (tab, doc) = pane.get_focused_tab_with_data_mut(doc_list)?;
                let position = range.decode(encoding, doc).start;

                doc.jump_cursors(position, false, ctx.gfx);
                tab.camera.recenter();
            }
            MessageResult::SignatureHelp(signature_help) => {
                editor
                    .signature_help_popup
                    .lsp_set_signature_help(signature_help, ctx);
            }
            MessageResult::Hover(hover) => {
                let doc = doc?;
                let hover = hover.map(|hover| hover.decode(encoding, doc));

                editor.lsp_set_hover(hover, doc_id?, ctx);
            }
            MessageResult::Formatting(edits) => {
                let doc = doc?;

                let mut edits: Vec<DecodedTextEdit> = edits
                    .into_iter()
                    .map(|edit| edit.decode(encoding, doc))
                    .collect();

                doc.lsp_apply_edit_list(&mut edits, ctx);
                let _ = doc.save(None, ctx);
            }
            MessageResult::Diagnostic(diagnostics) => {
                let doc = doc?;
                let path = doc.path().some()?.clone();

                server.set_diagnostics(path, diagnostics);
            }
        }

        Some(())
    }

    fn poll(&mut self) -> Option<(usize, Message)> {
        for (index, server) in self.servers.iter_mut() {
            let Some(server) = server else {
                continue;
            };

            if let Some(result) = server.poll() {
                return Some((*index, result));
            }
        }

        None
    }

    pub fn iter_servers_mut(&mut self) -> impl Iterator<Item = &mut LanguageServer> {
        self.servers.values_mut().flatten()
    }

    pub fn get_language_server_mut(
        &mut self,
        doc: &Doc,
        config: &Config,
    ) -> Option<&mut LanguageServer> {
        let current_dir = self.current_dir.as_ref()?;

        if doc
            .path()
            .some()
            .is_none_or(|path| !path.starts_with(current_dir))
        {
            return None;
        }

        let language = config.get_language_for_doc(doc)?;

        if let Entry::Vacant(entry) = self.servers.entry(language.index) {
            let command = language.lsp.command.as_ref()?;
            let language_server = LanguageServer::new(command, current_dir, &language.lsp.options);

            entry.insert(language_server);
        }

        let server = self.servers.get_mut(&language.index)?;
        server.as_mut()
    }

    pub fn get_diagnostic_at<'a>(
        &'a mut self,
        position: Position,
        doc: &Doc,
    ) -> Option<&'a mut DecodedDiagnostic> {
        for language_server in self.iter_servers_mut() {
            for diagnostic in language_server.diagnostics_mut(doc) {
                if !diagnostic.contains_position(position, doc) {
                    continue;
                }

                return Some(diagnostic);
            }
        }

        None
    }

    pub fn processes(&mut self) -> impl Iterator<Item = &mut Process> {
        self.iter_servers_mut().map(|server| &mut server.process)
    }
}
