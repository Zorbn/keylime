use core::str;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde_json::{json, Value};

use crate::{
    geometry::position::Position,
    lsp::{
        types::{
            CompletionItem, LspCompletionList, LspInitializeResult, LspMessageHeader, LspPosition,
            LspPublishDiagnosticsParams,
        },
        uri::uri_to_path,
    },
    platform::process::{Process, ProcessKind},
    temp_buffer::TempString,
    text::doc::Doc,
    ui::editor::Editor,
};

use super::{
    position_encoding::PositionEncoding,
    types::{Diagnostic, LspDiagnostic},
    uri::path_to_uri,
};

#[derive(Debug, Default)]
struct Diagnostics {
    encoded: Vec<LspDiagnostic>,
    decoded: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn replace(&mut self, encoded: &mut Vec<LspDiagnostic>) {
        self.decoded.clear();
        self.encoded.clear();

        self.encoded.append(encoded);
    }

    pub fn decode(&mut self, encoding: PositionEncoding, doc: &Doc) {
        self.decoded.extend(
            self.encoded
                .drain(..)
                .map(|diagnostic| diagnostic.decode(encoding, doc)),
        );
    }
}

enum MessageParseState {
    Idle,
    HasContentLen(usize),
}

pub struct LanguageServer {
    pub(super) process: Process,
    next_request_id: u64,
    pending_requests: HashMap<u64, &'static str>,
    parse_state: MessageParseState,
    message_queue: Vec<u8>,
    has_initialized: bool,
    uri_buffer: TempString,
    diagnostics: HashMap<PathBuf, Diagnostics>,
    position_encoding: PositionEncoding,
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
            position_encoding: PositionEncoding::Utf16,
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
                    },
                ],
                "capabilities": {
                    "workspace": {
                        "workspaceFolders": true,
                    },
                    "general": {
                        "positionEncodings": ["utf-8", "utf-16"],
                    },
                },
            }),
        );

        language_server.uri_buffer.replace(uri_buffer);

        Some(language_server)
    }

    pub fn get_diagnostics_mut(&mut self, doc: &Doc) -> &mut [Diagnostic] {
        let Some(path) = doc.path().on_drive() else {
            return &mut [];
        };

        self.diagnostics
            .get_mut(path)
            .map(|diagnostics| {
                diagnostics.decode(self.position_encoding, doc);
                diagnostics.decoded.as_mut_slice()
            })
            .unwrap_or_default()
    }

    pub fn update(&mut self, editor: &mut Editor) {
        loop {
            let (_, output) = self.process.input_output();

            let Ok(mut output) = output.lock() else {
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

                    let Ok(content_len) =
                        header[prefix_len..header_len - suffix_len].parse::<usize>()
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
                            if let Some(Ok(result)) = message.result.as_ref().map(|result| {
                                serde_json::from_str::<LspInitializeResult>(result.get())
                            }) {
                                if result.capabilities.position_encoding == "utf-8" {
                                    self.position_encoding = PositionEncoding::Utf8;
                                }
                            }

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

                            let uri = self.uri_buffer.get_mut();
                            uri.push_str(&params.uri);

                            let mut path = params.uri;
                            path.clear();

                            let Some(path) = uri_to_path(uri, path) else {
                                return;
                            };

                            let diagnostics = self.diagnostics.entry(path).or_default();
                            diagnostics.replace(&mut params.diagnostics);
                        }
                        "textDocument/completion" => {
                            let result = message.result.as_ref().map(|result| result.get());

                            let result = result
                                .and_then(|result| {
                                    serde_json::from_str::<LspCompletionList>(result).ok()
                                })
                                .map(|result| result.items)
                                .unwrap_or_default();

                            let Some((_, doc)) = editor.get_focused_tab_and_doc() else {
                                return;
                            };

                            // The compiler should perform an in-place collect here because
                            // LspCompletionItem and CompletionItem have the same size and alignment.
                            let mut result: Vec<CompletionItem> = result
                                .into_iter()
                                .map(|item| item.decode(self.position_encoding, doc))
                                .collect();

                            editor.lsp_add_completion_results(&mut result);
                        }
                        _ => {}
                    }
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

    pub fn did_change(
        &mut self,
        path: &Path,
        version: usize,
        start: Position,
        end: Position,
        text: &str,
        doc: &Doc,
    ) {
        let mut uri_buffer = self.uri_buffer.take_mut();

        path_to_uri(path, &mut uri_buffer);

        self.send_notification(
            "textDocument/didChange",
            json!({
                "textDocument": {
                    "uri": uri_buffer,
                    "version": version,
                },
                "contentChanges": [{
                    "text": text,
                    "range": {
                        "start": LspPosition::encode(start, self.position_encoding, doc),
                        "end": LspPosition::encode(end, self.position_encoding, doc),
                    }
                }]
            }),
        );

        self.uri_buffer.replace(uri_buffer);
    }

    pub fn completion(&mut self, path: &Path, position: Position, doc: &Doc) {
        let mut uri_buffer = self.uri_buffer.take_mut();

        path_to_uri(path, &mut uri_buffer);

        self.send_request(
            "textDocument/completion",
            json!({
                "textDocument": {
                    "uri": uri_buffer,
                },
                "position": LspPosition::encode(position, self.position_encoding, doc),
            }),
        );

        self.uri_buffer.replace(uri_buffer);
    }

    pub fn text_document_notification(&mut self, path: &Path, method: &'static str) {
        let mut uri_buffer = self.uri_buffer.take_mut();

        path_to_uri(path, &mut uri_buffer);

        self.send_notification(
            method,
            json!({
                "textDocument": {
                    "uri": uri_buffer,
                },
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
