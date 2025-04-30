pub mod language_server;
mod position_encoding;
pub mod types;
mod uri;

use std::{
    collections::{hash_map::Entry, HashMap},
    env::current_dir,
    path::PathBuf,
};

use language_server::{LanguageServer, LanguageServerResult};
use position_encoding::PositionEncoding;
use types::{CompletionItem, LspMessage};

use crate::{config::language::Language, ctx::Ctx, platform::process::Process, ui::editor::Editor};

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

    pub fn update_current_dir(&mut self) {
        self.current_dir = current_dir().ok();
        self.clear();
    }

    pub fn update(editor: &mut Editor, ctx: &mut Ctx) {
        while let Some((language_index, message)) = ctx.lsp.poll() {
            let Some((encoding, result)) = ctx.lsp.handle_message(language_index, &message) else {
                continue;
            };

            match result {
                LanguageServerResult::Completion(completion_items) => {
                    let Some((_, doc)) = editor.get_focused_tab_and_doc() else {
                        continue;
                    };

                    // The compiler should perform an in-place collect here because
                    // LspCompletionItem and CompletionItem have the same size and alignment.
                    let completion_items: Vec<CompletionItem> = completion_items
                        .into_iter()
                        .map(|item| item.decode(encoding, doc))
                        .collect();

                    editor.completion_list.lsp_update_results(completion_items);
                }
                LanguageServerResult::Definition { path, range } => {
                    let (pane, doc_list) = editor.get_focused_pane_and_doc_list();

                    if pane.open_file(&path, doc_list, ctx).is_err() {
                        continue;
                    }

                    let focused_tab_index = pane.focused_tab_index();
                    let Some((tab, doc)) = pane.get_tab_with_data_mut(focused_tab_index, doc_list)
                    else {
                        continue;
                    };

                    let (position, _) = range.decode(encoding, doc);

                    doc.jump_cursors(position, false, ctx.gfx);
                    tab.camera.recenter();
                }
            }
        }
    }

    fn poll(&mut self) -> Option<(usize, LspMessage)> {
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

    fn handle_message<'a>(
        &mut self,
        language_index: usize,
        message: &'a LspMessage,
    ) -> Option<(PositionEncoding, LanguageServerResult<'a>)> {
        let server = self.servers.get_mut(&language_index)?.as_mut()?;

        server
            .handle_message(message)
            .map(|result| (server.position_encoding(), result))
    }

    pub fn iter_servers_mut(&mut self) -> impl Iterator<Item = &mut LanguageServer> {
        self.servers.values_mut().flatten()
    }

    pub fn get_language_server_mut(&mut self, language: &Language) -> Option<&mut LanguageServer> {
        let current_dir = self.current_dir.as_ref()?;

        if let Entry::Vacant(entry) = self.servers.entry(language.index) {
            let language_server_command = language.language_server_command.as_ref()?;
            let language_server = LanguageServer::new(language_server_command, current_dir);

            entry.insert(language_server);
        }

        let server = self.servers.get_mut(&language.index)?;
        server.as_mut()
    }

    pub fn processes(&mut self) -> impl Iterator<Item = &mut Process> {
        self.iter_servers_mut().map(|server| &mut server.process)
    }
}
