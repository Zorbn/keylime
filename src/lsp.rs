use core::str;
use std::{
    collections::{hash_map::Entry, HashMap},
    env::current_dir,
    path::{Path, PathBuf},
};

use serde_json::{json, Map, Value};

use crate::{
    config::language::Language,
    platform::process::{Process, ProcessKind},
};

fn path_to_uri(path: &Path) -> String {
    assert!(path.is_absolute());

    let mut uri = "file:".to_string();

    if let Some(parent) = path.parent() {
        for component in parent {
            let Some(component) = component.to_str() else {
                continue;
            };

            encode_path_component(component, &mut uri);
            uri.push('/');
        }
    }

    if let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) {
        encode_path_component(file_name, &mut uri);
    }

    uri
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

    pub fn clear(&mut self) {
        self.servers.clear();
    }

    pub fn update_current_dir(&mut self) {
        self.current_dir = current_dir().ok();
        self.clear();
    }

    pub fn update(&mut self) {
        let Some(current_dir) = &self.current_dir else {
            return;
        };

        for server in self.servers.values_mut() {
            server.update(current_dir);
        }
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
}

impl LanguageServer {
    pub fn new(command: &str, current_dir: &Path) -> Option<Self> {
        let process = Process::new(&[command], ProcessKind::Normal).ok()?;

        let mut language_server = LanguageServer {
            process,
            next_request_id: 0,
            pending_requests: HashMap::new(),
            parse_state: MessageParseState::Idle,
        };

        language_server.send_request(
            "initialize",
            json!({
                "workspaceFolders": [
                    {
                        "uri": path_to_uri(current_dir),
                        "name": current_dir.file_name().and_then(|file_name| file_name.to_str()).unwrap_or_default()
                    }
                ],
                "capabilities": {
                    "workspace": {
                        "workspaceFolders": true
                    }
                }
            }),
        );

        Some(language_server)
    }

    pub fn update(&mut self, current_dir: &Path) {
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

                let Ok(message) =
                    serde_json::from_slice::<Map<String, Value>>(&output[..content_len])
                else {
                    output.drain(..content_len);
                    return;
                };

                output.drain(..content_len);
                drop(output);

                let method = message.get("id").and_then(|id| id.as_u64()).and_then(|id| {
                    self.pending_requests
                        .remove_entry(&id)
                        .map(|(_, method)| method)
                });

                let Some(method) =
                    method.or_else(|| message.get("method").and_then(|method| method.as_str()))
                else {
                    return;
                };

                match method {
                    "initialize" => {
                        self.send_notification("initialized", json!({}));
                    }
                    _ => {}
                }
            }
        }
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

        self.send_content(content);
    }

    fn send_notification(&mut self, method: &'static str, params: Value) {
        let content = json!({
            "jsonrpc": 2.0,
            "method": method,
            "params": params,
        });

        self.send_content(content);
    }

    fn send_content(&mut self, content: Value) {
        let content = format!("{}", content);
        let header = format!("Content-Length: {}\r\n\r\n", content.len());

        self.process.input().extend_from_slice(header.as_bytes());
        self.process.input().extend_from_slice(content.as_bytes());
        self.process.flush();
    }
}

impl Drop for LanguageServer {
    fn drop(&mut self) {
        self.send_request("shutdown", json!({}));
        self.send_notification("exit", json!({}));
    }
}
