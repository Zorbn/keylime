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
use types::LspMessage;

use crate::{config::language::Language, platform::process::Process};

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

    pub fn poll(&mut self) -> Option<(usize, LspMessage)> {
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

    pub fn handle_message<'a>(
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
