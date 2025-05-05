use core::str;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde_json::{json, value::RawValue, Value};

use crate::{
    geometry::position::Position,
    lsp::{
        types::{
            LspCodeActionResult, LspCompletionResult, LspDefinitionResult, LspInitializeResult,
            LspLocation, LspMessage, LspPosition, LspPublishDiagnosticsParams,
        },
        uri::uri_to_path,
    },
    platform::process::{Process, ProcessKind},
    temp_buffer::TempString,
    text::doc::Doc,
};

use super::{
    position_encoding::PositionEncoding,
    types::{
        CompletionItem, Diagnostic, LspCompletionItem, LspDiagnostic, LspPrepareRenameResult,
        LspRange, LspWorkspaceEdit,
    },
    uri::path_to_uri,
    LspSentRequest,
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
        self.encoded.sort_by(|a, b| a.severity.cmp(&b.severity));
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

#[derive(Debug)]
pub(super) enum MessageResult<'a> {
    Completion(Vec<LspCompletionItem>),
    CompletionItemResolve(LspCompletionItem),
    CodeAction(Vec<LspCodeActionResult<'a>>),
    PrepareRename {
        range: LspRange,
        placeholder: Option<String>,
    },
    Rename(LspWorkspaceEdit<'a>),
    References(Vec<LspLocation<'a>>),
    Definition {
        path: PathBuf,
        range: LspRange,
    },
}

pub struct LanguageServer {
    pub(super) process: Process,
    next_request_id: usize,
    pending_requests: HashMap<usize, &'static str>,
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
                    "textDocument": {
                        "codeAction": {
                            "codeActionLiteralSupport": {
                                "codeActionKind": {
                                    "valueSet": ["", "quickfix", "refactor", "source"],
                                }
                            },
                            "isPreferredSupport": true,
                        },
                        "rename": {
                            "prepareSupport": true,
                        },
                        "completion": {
                            "completionItem": {
                                "resolveSupport": {
                                    "properties": ["documentation", "textEdit", "additionalTextEdits"],
                                },
                                "labelDetailsSupport": true,
                            },
                        },
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

    pub(super) fn poll(&mut self) -> Option<LspMessage> {
        loop {
            let (_, output) = self.process.input_output();
            let mut output = output.lock().ok()?;

            match self.parse_state {
                MessageParseState::Idle => {
                    let mut header_len = None;

                    for i in 3..output.len() {
                        if &output[i - 3..=i] == b"\r\n\r\n" {
                            header_len = Some(i + 1);
                            break;
                        }
                    }

                    let header_len = header_len?;
                    let header = str::from_utf8(&output[..header_len]);

                    let Ok(header) = header else {
                        output.drain(..header_len);
                        return None;
                    };

                    let prefix_len = "Content-Length: ".len();
                    let suffix_len = "\r\n\r\n".len();

                    if header_len < prefix_len + suffix_len {
                        output.drain(..header_len);
                        return None;
                    }

                    let Ok(content_len) =
                        header[prefix_len..header_len - suffix_len].parse::<usize>()
                    else {
                        output.drain(..header_len);
                        return None;
                    };

                    self.parse_state = MessageParseState::HasContentLen(content_len);
                    output.drain(..header_len);
                }
                MessageParseState::HasContentLen(content_len) => {
                    if output.len() < content_len {
                        return None;
                    }

                    self.parse_state = MessageParseState::Idle;

                    let message = serde_json::from_slice::<LspMessage>(&output[..content_len]);

                    #[cfg(feature = "lsp_debug")]
                    println!("{:?}", message);

                    output.drain(..content_len);

                    return message.ok();
                }
            }
        }
    }

    pub(super) fn get_message_method<'a>(&mut self, message: &'a LspMessage) -> Option<&'a str> {
        message
            .id
            .and_then(|id| {
                self.pending_requests
                    .remove_entry(&id)
                    .map(|(_, method)| method)
            })
            .or(message.method.as_deref())
    }

    pub(super) fn handle_message<'a>(
        &mut self,
        method: &'a str,
        message: &'a LspMessage,
    ) -> Option<MessageResult<'a>> {
        match method {
            "initialize" => {
                let result = message.result.as_ref()?;

                if let Ok(result) = serde_json::from_str::<LspInitializeResult>(result.get()) {
                    if result.capabilities.position_encoding == "utf-8" {
                        self.position_encoding = PositionEncoding::Utf8;
                    }
                }

                self.send_notification("initialized", json!({}));

                self.has_initialized = true;

                self.process.input().extend_from_slice(&self.message_queue);
                self.process.flush();

                self.message_queue.clear();

                None
            }
            "textDocument/publishDiagnostics" => {
                let params = message.params.as_ref()?;

                let mut params =
                    serde_json::from_str::<LspPublishDiagnosticsParams>(params.get()).ok()?;

                let uri = self.uri_buffer.get_mut();
                uri.push_str(&params.uri);

                let mut path = params.uri;
                path.clear();

                let path = uri_to_path(uri, path)?;

                let diagnostics = self.diagnostics.entry(path).or_default();
                diagnostics.replace(&mut params.diagnostics);

                None
            }
            "textDocument/completion" => {
                let result = message
                    .result
                    .as_ref()
                    .and_then(|result| {
                        serde_json::from_str::<LspCompletionResult>(result.get()).ok()
                    })
                    .and_then(|result| match result {
                        LspCompletionResult::None => None,
                        LspCompletionResult::Items(items) => Some(items),
                        LspCompletionResult::List(list) => Some(list.items),
                    })
                    .unwrap_or_default();

                Some(MessageResult::Completion(result))
            }
            "completionItem/resolve" => {
                let result = message.result.as_ref()?;
                let result = serde_json::from_str::<LspCompletionItem>(result.get()).ok()?;

                Some(MessageResult::CompletionItemResolve(result))
            }
            "textDocument/codeAction" => {
                let result = message
                    .result
                    .as_deref()
                    .and_then(|result| {
                        serde_json::from_str::<Vec<LspCodeActionResult>>(result.get()).ok()
                    })
                    .unwrap_or_default();

                Some(MessageResult::CodeAction(result))
            }
            "textDocument/prepareRename" => {
                let result = message.result.as_ref()?;

                let result = serde_json::from_str::<LspPrepareRenameResult>(result.get())
                    .ok()
                    .unwrap_or_default();

                match result {
                    LspPrepareRenameResult::Range(range) => Some(MessageResult::PrepareRename {
                        range,
                        placeholder: None,
                    }),
                    LspPrepareRenameResult::RangeWithPlaceholder { range, placeholder } => {
                        Some(MessageResult::PrepareRename {
                            range,
                            placeholder: Some(placeholder),
                        })
                    }
                    LspPrepareRenameResult::Invalid => None,
                }
            }
            "textDocument/rename" => {
                let result = message.result.as_ref()?;
                let result = serde_json::from_str::<LspWorkspaceEdit>(result.get()).ok();

                result.map(MessageResult::Rename)
            }
            "textDocument/references" => {
                let result = message.result.as_ref()?;
                let result = serde_json::from_str::<Vec<LspLocation>>(result.get()).ok();

                result.map(MessageResult::References)
            }
            "textDocument/definition" => {
                let result = message.result.as_ref()?;

                let result = serde_json::from_str::<LspDefinitionResult>(result.get())
                    .ok()
                    .and_then(|result| match result {
                        LspDefinitionResult::None => None,
                        LspDefinitionResult::Location(location) => Some(location),
                        LspDefinitionResult::Locations(locations) => locations.into_iter().nth(0),
                        LspDefinitionResult::Links(links) => {
                            links.into_iter().nth(0).map(|link| LspLocation {
                                uri: link.target_uri,
                                range: link.target_range,
                            })
                        }
                    })?;

                let path = uri_to_path(result.uri, String::new())?;

                Some(MessageResult::Definition {
                    path,
                    range: result.range,
                })
            }
            _ => None,
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

    pub fn completion(&mut self, path: &Path, position: Position, doc: &Doc) -> LspSentRequest {
        let mut uri_buffer = self.uri_buffer.take_mut();

        path_to_uri(path, &mut uri_buffer);

        let sent_request = self.send_request(
            "textDocument/completion",
            json!({
                "textDocument": {
                    "uri": uri_buffer,
                },
                "position": LspPosition::encode(position, self.position_encoding, doc),
            }),
        );

        self.uri_buffer.replace(uri_buffer);

        sent_request
    }

    pub fn completion_item_resolve(&mut self, item: CompletionItem, doc: &Doc) -> LspSentRequest {
        self.send_request(
            "completionItem/resolve",
            json!(item.encode(self.position_encoding, doc)),
        )
    }

    pub fn code_action(
        &mut self,
        path: &Path,
        start: Position,
        end: Position,
        doc: &Doc,
    ) -> LspSentRequest {
        let mut uri_buffer = self.uri_buffer.take_mut();

        path_to_uri(path, &mut uri_buffer);

        let encoding = self.position_encoding;

        let sent_reqest = self.send_request(
            "textDocument/codeAction",
            json!({
                "textDocument": {
                    "uri": uri_buffer,
                },
                "range": {
                    "start": LspPosition::encode(start, encoding, doc),
                    "end": LspPosition::encode(end, encoding, doc),
                },
                "context": {
                    "diagnostics": [],
                },
            }),
        );

        self.uri_buffer.replace(uri_buffer);

        sent_reqest
    }

    pub fn prepare_rename(&mut self, path: &Path, position: Position, doc: &Doc) -> LspSentRequest {
        let mut uri_buffer = self.uri_buffer.take_mut();

        path_to_uri(path, &mut uri_buffer);

        let sent_reqest = self.send_request(
            "textDocument/prepareRename",
            json!({
                "textDocument": {
                    "uri": uri_buffer,
                },
                "position": LspPosition::encode(position, self.position_encoding, doc),
            }),
        );

        self.uri_buffer.replace(uri_buffer);

        sent_reqest
    }

    pub fn rename(
        &mut self,
        new_name: &str,
        path: &Path,
        position: Position,
        doc: &Doc,
    ) -> LspSentRequest {
        let mut uri_buffer = self.uri_buffer.take_mut();

        path_to_uri(path, &mut uri_buffer);

        let sent_request = self.send_request(
            "textDocument/rename",
            json!({
                "textDocument": {
                    "uri": uri_buffer,
                },
                "position": LspPosition::encode(position, self.position_encoding, doc),
                "newName": new_name,
            }),
        );

        self.uri_buffer.replace(uri_buffer);

        sent_request
    }

    pub fn references(&mut self, path: &Path, position: Position, doc: &Doc) -> LspSentRequest {
        let mut uri_buffer = self.uri_buffer.take_mut();

        path_to_uri(path, &mut uri_buffer);

        let sent_request = self.send_request(
            "textDocument/references",
            json!({
                "textDocument": {
                    "uri": uri_buffer,
                },
                "position": LspPosition::encode(position, self.position_encoding, doc),
                "context": {
                    "includeDeclaration": true,
                },
            }),
        );

        self.uri_buffer.replace(uri_buffer);

        sent_request
    }

    pub fn execute_command(&mut self, command: &str, arguments: &[Box<RawValue>]) {
        self.send_request(
            "workspace/executeCommand",
            json!({
                "command": command,
                "arguments": arguments,
            }),
        );
    }

    pub fn definition(&mut self, path: &Path, position: Position, doc: &Doc) -> LspSentRequest {
        let mut uri_buffer = self.uri_buffer.take_mut();

        path_to_uri(path, &mut uri_buffer);

        let sent_request = self.send_request(
            "textDocument/definition",
            json!({
                "textDocument": {
                    "uri": uri_buffer,
                },
                "position": LspPosition::encode(position, self.position_encoding, doc),
            }),
        );

        self.uri_buffer.replace(uri_buffer);

        sent_request
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

    fn send_request(&mut self, method: &'static str, params: Value) -> LspSentRequest {
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

        LspSentRequest { method, id }
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

    pub fn position_encoding(&self) -> PositionEncoding {
        self.position_encoding
    }
}

impl Drop for LanguageServer {
    fn drop(&mut self) {
        self.send_request("shutdown", json!({}));
        self.send_notification("exit", json!({}));
    }
}
