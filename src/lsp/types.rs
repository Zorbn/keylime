use std::{borrow::Cow, collections::HashMap};

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::{config::theme::Theme, geometry::position::Position, text::doc::Doc, ui::color::Color};

use super::position_encoding::PositionEncoding;

const DEFAULT_SEVERITY: fn() -> usize = || 1;

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub(super) struct LspPosition {
    line: usize,
    character: usize,
}

impl LspPosition {
    pub fn encode(position: Position, encoding: PositionEncoding, doc: &Doc) -> LspPosition {
        let line = doc.get_line(position.y).unwrap_or_default();

        match encoding {
            PositionEncoding::Utf8 => LspPosition {
                line: position.y,
                character: position.x,
            },
            PositionEncoding::Utf16 => LspPosition {
                line: position.y,
                character: line[..position.x].encode_utf16().count(),
            },
        }
    }

    fn decode(self, encoding: PositionEncoding, doc: &Doc) -> Position {
        let line = doc.get_line(self.line).unwrap_or_default();

        match encoding {
            PositionEncoding::Utf8 => Position {
                x: self.character,
                y: self.line,
            },
            PositionEncoding::Utf16 => {
                let mut wide_index = 0;
                let mut result = Position {
                    x: line.len(),
                    y: self.line,
                };

                for (index, c) in line.char_indices() {
                    let mut dst = [0; 2];

                    wide_index += c.encode_utf16(&mut dst).iter().count();

                    if wide_index >= self.character {
                        result.x = index;
                        break;
                    }
                }

                result
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub(super) struct LspRange {
    start: LspPosition,
    end: LspPosition,
}

impl LspRange {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> (Position, Position) {
        let start = self.start.decode(encoding, doc);
        let end = self.end.decode(encoding, doc);

        (start, end)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct LspDiagnostic {
    message: String,
    range: LspRange,
    #[serde(default = "DEFAULT_SEVERITY")]
    pub severity: usize,
}

impl LspDiagnostic {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> Diagnostic {
        Diagnostic {
            message: self.message,
            range: self.range.decode(encoding, doc),
            severity: self.severity,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct LspPublishDiagnosticsParams {
    pub uri: String,
    pub diagnostics: Vec<LspDiagnostic>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LspTextEdit<'a> {
    range: LspRange,
    new_text: Cow<'a, str>,
}

impl<'a> LspTextEdit<'a> {
    fn decode(self, encoding: PositionEncoding, doc: &Doc) -> TextEdit<'a> {
        TextEdit {
            range: self.range.decode(encoding, doc),
            new_text: self.new_text,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LspCompletionItem<'a> {
    label: &'a str,
    sort_text: Option<&'a str>,
    filter_text: Option<&'a str>,
    insert_text: Option<Cow<'a, str>>,
    text_edit: Option<LspTextEdit<'a>>,
}

impl<'a> LspCompletionItem<'a> {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> CompletionItem<'a> {
        CompletionItem {
            label: self.label,
            sort_text: self.sort_text,
            filter_text: self.filter_text,
            insert_text: self.insert_text,
            text_edit: self
                .text_edit
                .map(|text_edit| text_edit.decode(encoding, doc)),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct LspCompletionList<'a> {
    #[serde(borrow)]
    pub items: Vec<LspCompletionItem<'a>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum LspCompletionResult<'a> {
    List(LspCompletionList<'a>),
    #[serde(borrow)]
    Items(Vec<LspCompletionItem<'a>>),
    None,
}

#[derive(Debug, Deserialize)]
pub(super) struct LspLocation<'a> {
    pub uri: &'a str,
    pub range: LspRange,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LspLocationLink<'a> {
    pub target_uri: &'a str,
    pub target_range: LspRange,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum LspDefinitionResult<'a> {
    #[serde(borrow)]
    Location(LspLocation<'a>),
    Locations(Vec<LspLocation<'a>>),
    Links(Vec<LspLocationLink<'a>>),
    None,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LspServerCapabilities<'a> {
    pub position_encoding: &'a str,
}

#[derive(Debug, Deserialize)]
pub(super) struct LspInitializeResult<'a> {
    #[serde(borrow)]
    pub capabilities: LspServerCapabilities<'a>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LspMessage {
    pub id: Option<u64>,
    pub method: Option<String>,
    pub result: Option<Box<RawValue>>,
    pub params: Option<Box<RawValue>>,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub range: (Position, Position),
    pub severity: usize,
}

impl Diagnostic {
    pub fn color(&self, theme: &Theme) -> Color {
        match self.severity {
            1 => theme.error,
            2 => theme.warning,
            _ => theme.info,
        }
    }
}

#[derive(Debug)]
pub struct TextEdit<'a> {
    pub range: (Position, Position),
    pub new_text: Cow<'a, str>,
}

#[derive(Debug)]
pub struct CompletionItem<'a> {
    pub label: &'a str,
    sort_text: Option<&'a str>,
    filter_text: Option<&'a str>,
    pub insert_text: Option<Cow<'a, str>>,
    pub text_edit: Option<TextEdit<'a>>,
}

impl CompletionItem<'_> {
    pub fn sort_text(&self) -> &str {
        self.sort_text.unwrap_or(self.label)
    }

    pub fn filter_text(&self) -> &str {
        self.filter_text.unwrap_or(self.label)
    }
}

#[derive(Debug, Deserialize)]
struct LspTextDocumentIdentifier<'a> {
    pub uri: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LspTextDocumentEdit<'a> {
    #[serde(borrow)]
    text_document: LspTextDocumentIdentifier<'a>,
    edits: Vec<LspTextEdit<'a>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LspWorkspaceEdit<'a> {
    #[serde(borrow)]
    changes: Option<HashMap<&'a str, Vec<LspTextEdit<'a>>>>,
    document_changes: Option<Vec<LspTextDocumentEdit<'a>>>,
}

#[derive(Debug, Deserialize)]
pub struct Command<'a> {
    pub title: &'a str,
    pub command: &'a str,
    #[serde(default)]
    pub arguments: Vec<Box<RawValue>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LspCodeAction<'a> {
    title: &'a str,
    edit: Option<LspWorkspaceEdit<'a>>,
    command: Option<Command<'a>>,
    #[serde(default)]
    is_preferred: bool,
}

impl<'a> LspCodeAction<'a> {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> CodeAction<'a> {
        let mut edit = Vec::new();

        if let Some(lsp_edit) = self.edit {
            if let Some(changes) = lsp_edit.document_changes {
                for change in changes {
                    let edits: Vec<TextEdit<'a>> = change
                        .edits
                        .into_iter()
                        .map(|text_edit| text_edit.decode(encoding, doc))
                        .collect();

                    edit.push((change.text_document.uri, edits));
                }
            } else if let Some(changes) = lsp_edit.changes {
                for (uri, edits) in changes {
                    let edits: Vec<TextEdit<'a>> = edits
                        .into_iter()
                        .map(|text_edit| text_edit.decode(encoding, doc))
                        .collect();

                    edit.push((uri, edits));
                }
            }
        }

        CodeAction {
            title: self.title,
            edit,
            command: self.command,
            is_preferred: self.is_preferred,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum LspCodeActionResult<'a> {
    #[serde(borrow)]
    Command(Command<'a>),
    CodeAction(LspCodeAction<'a>),
    None,
}

impl<'a> LspCodeActionResult<'a> {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> CodeActionResult<'a> {
        match self {
            LspCodeActionResult::Command(command) => CodeActionResult::Command(command),
            LspCodeActionResult::CodeAction(code_action) => {
                CodeActionResult::CodeAction(code_action.decode(encoding, doc))
            }
            LspCodeActionResult::None => panic!(),
        }
    }
}

#[derive(Debug)]
pub struct CodeAction<'a> {
    pub title: &'a str,
    pub edit: Vec<(&'a str, Vec<TextEdit<'a>>)>,
    pub command: Option<Command<'a>>,
    pub is_preferred: bool,
}

#[derive(Debug)]
pub enum CodeActionResult<'a> {
    Command(Command<'a>),
    CodeAction(CodeAction<'a>),
}
