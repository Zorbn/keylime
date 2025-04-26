use core::str;
use std::{
    collections::{hash_map::Entry, HashMap},
    env::current_dir,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::{json, value::RawValue, Value};

use crate::{
    config::language::Language,
    geometry::position::Position,
    platform::process::{Process, ProcessKind},
    temp_buffer::TempString,
};

fn path_to_uri(path: &Path, result: &mut String) {
    assert!(path.is_absolute());

    result.push_str("file://");

    if let Some(parent) = path.parent() {
        for component in parent {
            let Some(component) = component.to_str() else {
                continue;
            };

            encode_path_component(component, result);

            if component != "/" {
                result.push('/');
            }
        }
    }

    if let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) {
        encode_path_component(file_name, result);
    }
}

fn encode_path_component(component: &str, result: &mut String) {
    for c in component.chars() {
        if c == ' ' {
            result.push_str("%20");
        } else {
            result.push(c);
        }
    }
}

const DEFAULT_SEVERITY: fn() -> usize = || 1;

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct LspPosition {
    line: usize,
    character: usize,
}

impl From<LspPosition> for Position {
    fn from(val: LspPosition) -> Self {
        Position {
            x: val.character,
            y: val.line,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

#[derive(Debug, Deserialize)]
pub struct LspDiagnostic {
    pub message: String,
    pub range: LspRange,
    #[serde(default = "DEFAULT_SEVERITY")]
    pub severity: usize,
}

#[derive(Debug, Deserialize)]
struct LspPublishDiagnosticsParams {
    uri: String,
    diagnostics: Vec<LspDiagnostic>,
}

#[derive(Debug, Deserialize)]
struct LspMessageHeader {
    id: Option<u64>,
    method: Option<String>,
    params: Option<Box<RawValue>>,
}

pub struct Lsp {
    servers: HashMap<usize, LanguageServer>,
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

    pub fn update(&mut self) {
        for server in self.servers.values_mut() {
            server.update();
        }
    }

    pub fn iter_servers_mut(&mut self) -> impl Iterator<Item = &mut LanguageServer> {
        self.servers.values_mut()
    }

    pub fn get_language_server_mut(&mut self, language: &Language) -> Option<&mut LanguageServer> {
        let current_dir = self.current_dir.as_ref()?;

        if let Entry::Vacant(entry) = self.servers.entry(language.index) {
            let language_server_command = language.language_server_command.as_ref()?;
            let language_server = LanguageServer::new(language_server_command, current_dir)?;

            entry.insert(language_server);
        }

        self.servers.get_mut(&language.index)
    }

    pub fn processes(&mut self) -> impl Iterator<Item = &mut Process> {
        self.servers.values_mut().map(|server| &mut server.process)
    }
}

enum MessageParseState {
    Idle,
    HasContentLen(usize),
}

pub struct LanguageServer {
    process: Process,
    next_request_id: u64,
    pending_requests: HashMap<u64, &'static str>,
    parse_state: MessageParseState,
    message_queue: Vec<u8>,
    has_initialized: bool,
    uri_buffer: TempString,
    diagnostics: HashMap<String, Vec<LspDiagnostic>>,
}

impl LanguageServer {
    pub fn new(command: &str, current_dir: &Path) -> Option<Self> {
        let process = Process::new(&[command], ProcessKind::Normal).ok()?;

        let mut language_server = LanguageServer {
            process,
            next_request_id: 0,
            pending_requests: HashMap::new(),
            parse_state: MessageParseState::Idle,
            message_queue: Vec::new(),
            has_initialized: false,
            uri_buffer: TempString::new(),
            diagnostics: HashMap::new(),
        };

        let workspace_name = current_dir
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or_default();

        let mut uri_buffer = language_server.uri_buffer.take_mut();

        path_to_uri(current_dir, &mut uri_buffer);

        language_server.send_request(
            "initialize",
            json!({
                "workspaceFolders": [
                    {
                        "uri": uri_buffer,
                        "name": workspace_name,
                    }
                ],
                "capabilities": {
                    "workspace": {
                        "workspaceFolders": true,
                    },
                },
            }),
        );

        language_server.uri_buffer.replace(uri_buffer);

        Some(language_server)
    }

    pub fn get_diagnostics(&mut self, path: &Path) -> &[LspDiagnostic] {
        let uri_buffer = self.uri_buffer.get_mut();

        path_to_uri(path, uri_buffer);

        self.diagnostics
            .get(uri_buffer)
            .map(|diagnostics| diagnostics.as_slice())
            .unwrap_or_default()
    }

    pub fn update(&mut self) {
        let (_, output) = self.process.input_output();

        let Ok(mut output) = output.try_lock() else {
            return;
        };

        match self.parse_state {
            MessageParseState::Idle => {
                let mut header_len = None;

                for i in 3..output.len() {
                    if &output[i - 3..=i] == b"\r\n\r\n" {
                        header_len = Some(i + 1);
                        break;
                    }
                }

                let Some(header_len) = header_len else {
                    return;
                };

                let header = str::from_utf8(&output[..header_len]);

                let Ok(header) = header else {
                    output.drain(..header_len);
                    return;
                };

                let prefix_len = "Content-Length: ".len();
                let suffix_len = "\r\n\r\n".len();

                if header_len < prefix_len + suffix_len {
                    output.drain(..header_len);
                    return;
                }

                let Ok(content_len) = header[prefix_len..header_len - suffix_len].parse::<usize>()
                else {
                    output.drain(..header_len);
                    return;
                };

                self.parse_state = MessageParseState::HasContentLen(content_len);
                output.drain(..header_len);
            }
            MessageParseState::HasContentLen(content_len) => {
                if output.len() < content_len {
                    return;
                }

                self.parse_state = MessageParseState::Idle;

                let Ok(message) =
                    serde_json::from_slice::<LspMessageHeader>(&output[..content_len])
                else {
                    output.drain(..content_len);
                    return;
                };

                #[cfg(feature = "lsp_debug")]
                println!("{:?}", message);

                output.drain(..content_len);
                drop(output);

                let Some(method) = message
                    .id
                    .and_then(|id| {
                        self.pending_requests
                            .remove_entry(&id)
                            .map(|(_, method)| method)
                    })
                    .or(message.method.as_deref())
                else {
                    return;
                };

                match method {
                    "initialize" => {
                        self.send_notification("initialized", json!({}));

                        self.has_initialized = true;

                        self.process.input().extend_from_slice(&self.message_queue);
                        self.process.flush();

                        self.message_queue.clear();
                    }
                    "textDocument/publishDiagnostics" => {
                        let Some(Ok(mut params)) = message.params.map(|params| {
                            serde_json::from_str::<LspPublishDiagnosticsParams>(params.get())
                        }) else {
                            return;
                        };

                        if !self.diagnostics.contains_key(&params.uri) {
                            self.diagnostics.insert(params.uri.clone(), Vec::new());
                        }

                        let diagnostics = self.diagnostics.get_mut(&params.uri).unwrap();

                        diagnostics.clear();
                        diagnostics.append(&mut params.diagnostics);
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn did_open(&mut self, path: &Path, language_id: &str, version: usize, text: &str) {
        let mut uri_buffer = self.uri_buffer.take_mut();

        path_to_uri(path, &mut uri_buffer);

        self.send_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri_buffer,
                    "languageId": language_id,
                    "version": version,
                    "text": text,
                }
            }),
        );

        self.uri_buffer.replace(uri_buffer);
    }

    fn send_request(&mut self, method: &'static str, params: Value) {
        let id = self.next_request_id;
        self.next_request_id += 1;

        self.pending_requests.insert(id, method);

        let content = json!({
            "jsonrpc": 2.0,
            "id": id,
            "method": method,
            "params": params,
        });

        self.send_content(content, self.has_initialized || method == "initialize");
    }

    fn send_notification(&mut self, method: &'static str, params: Value) {
        let content = json!({
            "jsonrpc": 2.0,
            "method": method,
            "params": params,
        });

        self.send_content(content, self.has_initialized || method == "initialized");
    }

    fn send_content(&mut self, content: Value, do_enqueue: bool) {
        let content = format!("{}", content);
        let header = format!("Content-Length: {}\r\n\r\n", content.len());

        let destination = if do_enqueue {
            self.process.input()
        } else {
            &mut self.message_queue
        };

        destination.extend_from_slice(header.as_bytes());
        destination.extend_from_slice(content.as_bytes());

        if do_enqueue {
            self.process.flush();
        }
    }
}

impl Drop for LanguageServer {
    fn drop(&mut self) {
        self.send_request("shutdown", json!({}));
        self.send_notification("exit", json!({}));
    }
}
