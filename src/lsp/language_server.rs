use core::str;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use serde_json::{json, value::RawValue, Value};

use crate::{
    config::language::IndentWidth,
    geometry::position::Position,
    lsp::{
        types::{
            EncodedDefinitionResult, EncodedFullDocumentDiagnosticParams, EncodedLocation,
            EncodedPosition, EncodedPublishDiagnosticsParams, InitializeResult,
            LspCodeActionResult, LspCompletionResult, Message, RegistrationParams,
        },
        uri::uri_to_path,
    },
    platform::{
        gfx::TAB_WIDTH,
        process::{Process, ProcessKind},
    },
    pool::{format_pooled, Pooled},
    text::doc::Doc,
};

use super::{
    position_encoding::PositionEncoding,
    types::{
        DecodedCompletionItem, DecodedDiagnostic, EncodedCompletionItem, EncodedDiagnostic,
        EncodedHover, EncodedRange, EncodedTextEdit, EncodedWorkspaceEdit, LspPrepareRenameResult,
        SignatureHelp,
    },
    uri::path_to_uri,
    LspSentRequest,
};

#[derive(Debug, Default)]
pub struct Diagnostics {
    encoded: Vec<EncodedDiagnostic>,
    decoded: Vec<DecodedDiagnostic>,
}

impl Diagnostics {
    pub(super) fn replace(&mut self, encoded: &mut Vec<EncodedDiagnostic>) {
        self.decoded.clear();
        self.encoded.clear();

        self.encoded.append(encoded);
        self.encoded.sort_by(|a, b| a.severity.cmp(&b.severity));
    }

    pub fn decode(&mut self, encoding: PositionEncoding, doc: &Doc) -> &mut [DecodedDiagnostic] {
        self.decoded.extend(
            self.encoded
                .drain(..)
                .map(|diagnostic| diagnostic.decode(encoding, doc)),
        );

        &mut self.decoded
    }

    pub fn encoded(&self) -> &[EncodedDiagnostic] {
        &self.encoded
    }

    pub fn decoded(&self) -> &[DecodedDiagnostic] {
        &self.decoded
    }
}

enum MessageParseState {
    Idle,
    HasContentLen(usize),
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub(super) enum MessageResult<'a> {
    Completion(Vec<EncodedCompletionItem>),
    CompletionItemResolve(EncodedCompletionItem),
    CodeAction(Vec<LspCodeActionResult>),
    PrepareRename {
        range: EncodedRange,
        placeholder: Option<Pooled<String>>,
    },
    Rename(EncodedWorkspaceEdit),
    References(Vec<EncodedLocation<'a>>),
    Definition {
        path: Pooled<PathBuf>,
        range: EncodedRange,
    },
    SignatureHelp(Option<SignatureHelp>),
    Hover(Option<EncodedHover>),
    Formatting(Vec<EncodedTextEdit>),
    Diagnostic(Vec<EncodedDiagnostic>),
}

pub struct LanguageServer {
    pub(super) process: Process,
    next_request_id: usize,
    pending_requests: HashMap<usize, (Option<Pooled<PathBuf>>, &'static str)>,
    parse_state: MessageParseState,
    message_queue: Vec<u8>,
    has_initialized: bool,

    diagnostics: HashMap<Pooled<PathBuf>, Diagnostics>,
    needs_completion_resolve: bool,
    do_pull_diagnostics: bool,

    position_encoding: PositionEncoding,
    trigger_chars: HashSet<char>,
    retrigger_chars: HashSet<char>,
}

impl LanguageServer {
    pub fn new(command: &str, current_dir: &Path, options: &Option<Value>) -> Option<Self> {
        let process = Process::new(&[command], ProcessKind::Normal).ok()?;

        let mut language_server = Self {
            process,
            next_request_id: 0,
            pending_requests: HashMap::new(),
            parse_state: MessageParseState::Idle,
            message_queue: Vec::new(),
            has_initialized: false,

            diagnostics: HashMap::new(),
            needs_completion_resolve: false,
            do_pull_diagnostics: false,

            position_encoding: PositionEncoding::Utf16,
            trigger_chars: HashSet::new(),
            retrigger_chars: HashSet::new(),
        };

        let workspace_name = current_dir
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or_default();

        let uri = path_to_uri(current_dir);
        let documentation_formats = ["plaintext", "markdown"];

        language_server.send_request(
            None,
            "initialize",
            json!({
                "initializationOptions": options,
                "rootUri": uri,
                "workspaceFolders": [
                    {
                        "uri": uri,
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
                                    "properties": ["documentation", "detail", "additionalTextEdits"],
                                },
                                "documentationFormat": documentation_formats,
                                "labelDetailsSupport": true,
                            },
                        },
                        "signatureHelp": {
                            "signatureInformation": {
                                "documentationFormat": documentation_formats,
                            },
                            "contextSupport": true,
                        },
                        "hover": {
                            "contentFormat": documentation_formats,
                        },
                        "definition": {
                            "linkSupport": true,
                        },
                        "diagnostic": {
                            "dynamicRegistration": true,
                        },
                    },
                },
            }),
        );

        Some(language_server)
    }

    pub fn diagnostics_mut(&mut self, doc: &Doc) -> &mut [DecodedDiagnostic] {
        let Some(path) = doc.path().some() else {
            return &mut [];
        };

        self.diagnostics
            .get_mut(path)
            .map(|diagnostics| diagnostics.decode(self.position_encoding, doc))
            .unwrap_or_default()
    }

    pub fn all_diagnostics_mut(
        &mut self,
    ) -> impl Iterator<Item = (&Pooled<PathBuf>, &mut Diagnostics)> {
        self.diagnostics.iter_mut()
    }

    pub(super) fn poll(&mut self) -> Option<Message> {
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

                    let message = serde_json::from_slice::<Message>(&output[..content_len]);

                    #[cfg(feature = "lsp_debug")]
                    println!("{:?}", message);

                    output.drain(..content_len);

                    return message.ok();
                }
            }
        }
    }

    pub(super) fn get_message_path_and_method<'a>(
        &mut self,
        message: &'a Message,
    ) -> (Option<Pooled<PathBuf>>, Option<&'a str>) {
        if let Some((_, (path, method))) = message
            .id
            .and_then(|id| self.pending_requests.remove_entry(&id))
        {
            return (path, Some(method));
        }

        (None, message.method.as_ref().map(|method| method.as_str()))
    }

    pub(super) fn handle_message<'a>(
        &mut self,
        method: &'a str,
        message: &'a Message,
    ) -> Option<MessageResult<'a>> {
        match method {
            "initialize" => {
                let result = message.result.as_ref()?;

                if let Ok(result) = serde_json::from_str::<InitializeResult>(result.get()) {
                    if result.capabilities.position_encoding == Some("utf-8") {
                        self.position_encoding = PositionEncoding::Utf8;
                    }

                    self.needs_completion_resolve = result
                        .capabilities
                        .completion_provider
                        .is_some_and(|provider| provider.resolve_provider);

                    if let Some(provider) = result.capabilities.signature_help_provider {
                        self.trigger_chars.extend(
                            provider
                                .trigger_characters
                                .iter()
                                .filter_map(|string| string.chars().nth(0)),
                        );

                        self.retrigger_chars.extend(
                            provider
                                .retrigger_characters
                                .iter()
                                .filter_map(|string| string.chars().nth(0)),
                        );
                    }

                    self.do_pull_diagnostics = result.capabilities.diagnostic_provider.is_some();
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
                let params =
                    serde_json::from_str::<EncodedPublishDiagnosticsParams>(params.get()).ok()?;

                let path = uri_to_path(&params.uri)?;

                self.set_diagnostics(path, params.diagnostics);

                None
            }
            "textDocument/diagnostic" => {
                let result = message.result.as_ref()?;
                let result =
                    serde_json::from_str::<EncodedFullDocumentDiagnosticParams>(result.get())
                        .ok()?;

                Some(MessageResult::Diagnostic(result.items))
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
                let result = serde_json::from_str::<EncodedCompletionItem>(result.get()).ok()?;

                Some(MessageResult::CompletionItemResolve(result))
            }
            "textDocument/codeAction" => {
                let result = message
                    .result
                    .as_ref()
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
                let result = serde_json::from_str::<EncodedWorkspaceEdit>(result.get()).ok();

                result.map(MessageResult::Rename)
            }
            "textDocument/references" => {
                let result = message.result.as_ref()?;
                let result = serde_json::from_str::<Vec<EncodedLocation>>(result.get()).ok();

                result.map(MessageResult::References)
            }
            "textDocument/definition" => {
                let result = message.result.as_ref()?;

                let result = serde_json::from_str::<EncodedDefinitionResult>(result.get())
                    .ok()
                    .and_then(|result| match result {
                        EncodedDefinitionResult::None => None,
                        EncodedDefinitionResult::Location(location) => Some(location),
                        EncodedDefinitionResult::Locations(locations) => {
                            locations.into_iter().nth(0)
                        }
                        EncodedDefinitionResult::Links(links) => {
                            links.into_iter().nth(0).map(|link| EncodedLocation {
                                uri: link.target_uri,
                                range: link.target_range,
                            })
                        }
                    })?;

                let path = uri_to_path(result.uri)?;

                Some(MessageResult::Definition {
                    path,
                    range: result.range,
                })
            }
            "textDocument/signatureHelp" => {
                let result = message
                    .result
                    .as_ref()
                    .and_then(|result| serde_json::from_str::<SignatureHelp>(result.get()).ok());

                Some(MessageResult::SignatureHelp(result))
            }
            "textDocument/hover" => {
                let result = message
                    .result
                    .as_ref()
                    .and_then(|result| serde_json::from_str::<EncodedHover>(result.get()).ok());

                Some(MessageResult::Hover(result))
            }
            "textDocument/formatting" => {
                let result = message.result.as_ref()?;
                let result = serde_json::from_str::<Vec<EncodedTextEdit>>(result.get()).ok();

                result.map(MessageResult::Formatting)
            }
            "client/registerCapability" => {
                let params = message.params.as_ref()?;
                let params = serde_json::from_str::<RegistrationParams>(params.get()).ok()?;

                for registration in params.registrations {
                    if registration.method == "textDocument/diagnostic" {
                        self.do_pull_diagnostics = true;
                    }
                }

                self.send_response(message.id?, json!({}));

                None
            }
            _ => None,
        }
    }

    pub(super) fn set_diagnostics(
        &mut self,
        path: Pooled<PathBuf>,
        mut diagnostics: Vec<EncodedDiagnostic>,
    ) {
        let path_diagnostics = self.diagnostics.entry(path).or_default();
        path_diagnostics.replace(&mut diagnostics);
    }

    pub fn did_open(&mut self, path: &Path, language_id: &str, version: usize, text: &str) {
        self.send_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                    "languageId": language_id,
                    "version": version,
                    "text": text,
                }
            }),
        );
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
        self.send_notification(
            "textDocument/didChange",
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                    "version": version,
                },
                "contentChanges": [{
                    "text": text,
                    "range": {
                        "start": EncodedPosition::encode(start, self.position_encoding, doc),
                        "end": EncodedPosition::encode(end, self.position_encoding, doc),
                    }
                }]
            }),
        );
    }

    pub fn completion(&mut self, path: &Path, position: Position, doc: &Doc) -> LspSentRequest {
        self.send_request(
            Some(path),
            "textDocument/completion",
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                },
                "position": EncodedPosition::encode(position, self.position_encoding, doc),
            }),
        )
    }

    pub fn completion_item_resolve(
        &mut self,
        item: DecodedCompletionItem,
        doc: &Doc,
    ) -> LspSentRequest {
        self.send_request(
            doc.path().some_path(),
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
        let encoding = self.position_encoding;

        let overlapping_diagnostic = self
            .diagnostics_mut(doc)
            .iter()
            .find(|DecodedDiagnostic { range, .. }| start <= range.end && end >= range.start)
            .map(|diagnostic| diagnostic.encode(encoding, doc));

        self.send_request(
            Some(path),
            "textDocument/codeAction",
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                },
                "range": {
                    "start": EncodedPosition::encode(start, encoding, doc),
                    "end": EncodedPosition::encode(end, encoding, doc),
                },
                "context": {
                    "diagnostics": overlapping_diagnostic.as_slice(),
                },
            }),
        )
    }

    pub fn prepare_rename(&mut self, path: &Path, position: Position, doc: &Doc) -> LspSentRequest {
        self.send_request(
            Some(path),
            "textDocument/prepareRename",
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                },
                "position": EncodedPosition::encode(position, self.position_encoding, doc),
            }),
        )
    }

    pub fn rename(
        &mut self,
        new_name: &str,
        path: &Path,
        position: Position,
        doc: &Doc,
    ) -> LspSentRequest {
        self.send_request(
            Some(path),
            "textDocument/rename",
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                },
                "position": EncodedPosition::encode(position, self.position_encoding, doc),
                "newName": new_name,
            }),
        )
    }

    pub fn references(&mut self, path: &Path, position: Position, doc: &Doc) -> LspSentRequest {
        self.send_request(
            Some(path),
            "textDocument/references",
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                },
                "position": EncodedPosition::encode(position, self.position_encoding, doc),
                "context": {
                    "includeDeclaration": true,
                },
            }),
        )
    }

    pub fn execute_command(&mut self, command: &str, arguments: &[Box<RawValue>]) {
        self.send_request(
            None,
            "workspace/executeCommand",
            json!({
                "command": command,
                "arguments": arguments,
            }),
        );
    }

    pub fn definition(&mut self, path: &Path, position: Position, doc: &Doc) -> LspSentRequest {
        self.send_request(
            Some(path),
            "textDocument/definition",
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                },
                "position": EncodedPosition::encode(position, self.position_encoding, doc),
            }),
        )
    }

    pub fn signature_help(
        &mut self,
        path: &Path,
        position: Position,
        trigger_char: Option<char>,
        is_retrigger: bool,
        doc: &Doc,
    ) -> LspSentRequest {
        self.send_request(
            Some(path),
            "textDocument/signatureHelp",
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                },
                "position": EncodedPosition::encode(position, self.position_encoding, doc),
                "context": {
                    "triggerKind": if trigger_char.is_some() {
                        2
                    } else {
                        3
                    },
                    "triggerCharacter": trigger_char,
                    "isRetrigger": is_retrigger,
                },
            }),
        )
    }

    pub fn hover(&mut self, path: &Path, position: Position, doc: &Doc) -> LspSentRequest {
        self.send_request(
            Some(path),
            "textDocument/hover",
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                },
                "position": EncodedPosition::encode(position, self.position_encoding, doc),
            }),
        )
    }

    pub fn formatting(&mut self, path: &Path, indent_width: IndentWidth) -> LspSentRequest {
        self.send_request(
            Some(path),
            "textDocument/formatting",
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                },
                "options": {
                    "tabSize": TAB_WIDTH,
                    "insertSpaces": matches!(indent_width, IndentWidth::Spaces(..)),
                },
            }),
        )
    }

    pub fn diagnostic(&mut self, path: &Path) -> Option<LspSentRequest> {
        if !self.do_pull_diagnostics {
            return None;
        }

        Some(self.send_request(
            Some(path),
            "textDocument/diagnostic",
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                },
            }),
        ))
    }

    pub fn text_document_notification(&mut self, path: &Path, method: &'static str) {
        self.send_notification(
            method,
            json!({
                "textDocument": {
                    "uri": path_to_uri(path),
                },
            }),
        );
    }

    fn send_request(
        &mut self,
        path: Option<&Path>,
        method: &'static str,
        params: Value,
    ) -> LspSentRequest {
        let id = self.next_request_id;
        self.next_request_id += 1;

        let path = path.map(Pooled::<PathBuf>::from);

        self.pending_requests.insert(id, (path, method));

        let content = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        self.send_content(content, self.has_initialized || method == "initialize");

        LspSentRequest { method, id }
    }

    fn send_notification(&mut self, method: &'static str, params: Value) {
        let content = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        self.send_content(content, self.has_initialized || method == "initialized");
    }

    fn send_response(&mut self, id: usize, result: Value) {
        let content = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        });

        self.send_content(content, self.has_initialized);
    }

    fn send_content(&mut self, content: Value, do_enqueue: bool) {
        let content = format_pooled!("{}", content);
        let header = format_pooled!("Content-Length: {}\r\n\r\n", content.len());

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

    pub fn needs_completion_resolve(&self) -> bool {
        self.needs_completion_resolve
    }

    pub fn is_trigger_char(&self, c: char) -> bool {
        self.trigger_chars.contains(&c)
    }

    pub fn is_retrigger_char(&self, c: char) -> bool {
        self.retrigger_chars.contains(&c)
    }
}

impl Drop for LanguageServer {
    fn drop(&mut self) {
        self.send_request(None, "shutdown", json!({}));
        self.send_notification("exit", json!({}));
    }
}
