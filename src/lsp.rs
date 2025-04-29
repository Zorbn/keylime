use core::str;
use std::{
    collections::{hash_map::Entry, HashMap},
    env::current_dir,
    iter::Peekable,
    path::{Path, PathBuf},
    str::Chars,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, value::RawValue, Value};

use crate::{
    config::{language::Language, theme::Theme},
    geometry::position::Position,
    platform::process::{Process, ProcessKind},
    temp_buffer::TempString,
    ui::{color::Color, editor::Editor},
};

const DEFAULT_SEVERITY: fn() -> usize = || 1;
const URI_SCHEME: &str = "file:///";

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct LspPosition {
    line: usize,
    character: usize,
}

impl From<LspPosition> for Position {
    fn from(position: LspPosition) -> Self {
        Position {
            x: position.character,
            y: position.line,
        }
    }
}

impl From<Position> for LspPosition {
    fn from(position: Position) -> Self {
        LspPosition {
            character: position.x,
            line: position.y,
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

impl LspDiagnostic {
    pub fn is_visible(&self) -> bool {
        self.severity != 4
    }

    pub fn color(&self, theme: &Theme) -> Color {
        match self.severity {
            1 => theme.error,
            2 => theme.warning,
            _ => theme.normal,
        }
    }
}

#[derive(Debug, Deserialize)]
struct LspPublishDiagnosticsParams {
    uri: String,
    diagnostics: Vec<LspDiagnostic>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspTextEdit<'a> {
    pub range: LspRange,
    pub new_text: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspCompletionItem<'a> {
    pub label: &'a str,
    sort_text: Option<&'a str>,
    filter_text: Option<&'a str>,
    pub insert_text: Option<&'a str>,
    pub text_edit: Option<LspTextEdit<'a>>,
}

impl LspCompletionItem<'_> {
    pub fn sort_text(&self) -> &str {
        self.sort_text.unwrap_or(self.label)
    }

    pub fn filter_text(&self) -> &str {
        self.filter_text.unwrap_or(self.label)
    }
}

#[derive(Debug, Deserialize)]
pub struct LspCompletionList<'a> {
    #[serde(borrow)]
    pub items: Vec<LspCompletionItem<'a>>,
}

#[derive(Debug, Deserialize)]
struct LspMessageHeader {
    id: Option<u64>,
    method: Option<String>,
    result: Option<Box<RawValue>>,
    params: Option<Box<RawValue>>,
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

    pub fn update_current_dir(&mut self) {
        self.current_dir = current_dir().ok();
        self.clear();
    }

    pub fn update(&mut self, editor: &mut Editor) {
        for server in self.iter_servers_mut() {
            server.update(editor);
        }
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
    diagnostics: HashMap<PathBuf, Vec<LspDiagnostic>>,
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
                    },
                ],
                "capabilities": {
                    "workspace": {
                        "workspaceFolders": true,
                    },
                    "general": {
                        "positionEncodings": ["utf-8"],
                    },
                },
            }),
        );

        language_server.uri_buffer.replace(uri_buffer);

        Some(language_server)
    }

    pub fn get_diagnostics_mut(&mut self, path: &Path) -> &mut [LspDiagnostic] {
        self.diagnostics
            .get_mut(path)
            .map(|diagnostics| diagnostics.as_mut_slice())
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
                            diagnostics.clear();
                            diagnostics.append(&mut params.diagnostics);
                        }
                        "textDocument/completion" => {
                            let result = message.result.as_ref().map(|result| result.get());

                            let mut result = result
                                .and_then(|result| {
                                    serde_json::from_str::<LspCompletionList>(result).ok()
                                })
                                .map(|result| result.items)
                                .unwrap_or_default();

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
                        "start": LspPosition::from(start),
                        "end": LspPosition::from(end),
                    }
                }]
            }),
        );

        self.uri_buffer.replace(uri_buffer);
    }

    pub fn completion(&mut self, path: &Path, position: Position) {
        let mut uri_buffer = self.uri_buffer.take_mut();

        path_to_uri(path, &mut uri_buffer);

        self.send_request(
            "textDocument/completion",
            json!({
                "textDocument": {
                    "uri": uri_buffer,
                },
                "position": LspPosition::from(position),
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

fn uri_to_path(uri: &str, mut result: String) -> Option<PathBuf> {
    if !uri.starts_with(URI_SCHEME) {
        return None;
    }

    let mut chars = uri[URI_SCHEME.len()..].chars().peekable();
    let mut c = chars.next();

    if let Some(first_char) = c {
        if first_char.is_ascii_alphabetic() && chars.peek() == Some(&':') {
            // This is a drive letter.
            result.push(first_char.to_ascii_uppercase());
            c = chars.next();
        } else {
            // No drive letter, add a root slash instead.
            result.push('/');
        }
    }

    while let Some(next_char) = c {
        result.push(match next_char {
            '%' => decode_uri_char(&mut chars)?,
            _ => next_char,
        });

        c = chars.next()
    }

    Some(PathBuf::from(result))
}

fn decode_uri_char(chars: &mut Peekable<Chars>) -> Option<char> {
    let first_digit = chars.next()?;
    let second_digit = chars.next()?;

    if !first_digit.is_ascii_hexdigit() || !second_digit.is_ascii_hexdigit() {
        return None;
    }

    let digits = &[first_digit as u8, second_digit as u8];
    let hex_string = str::from_utf8(digits).ok()?;

    u8::from_str_radix(hex_string, 16)
        .ok()
        .map(|value| value as char)
}

fn path_to_uri(path: &Path, result: &mut String) {
    assert!(path.is_absolute());

    result.push_str(URI_SCHEME);

    if let Some(parent) = path.parent() {
        for component in parent {
            let Some(component) = component.to_str() else {
                continue;
            };

            if matches!(component, "/" | "\\") {
                continue;
            }

            encode_path_component(component, result);
            result.push('/');
        }
    }

    if let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) {
        encode_path_component(file_name, result);
    }
}

fn encode_path_component(component: &str, result: &mut String) {
    for c in component.chars() {
        match c {
            ' ' => result.push_str("%20"),
            '\\' => result.push('/'),
            _ => result.push(c),
        }
    }
}
