use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{value::RawValue, Value};

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
    pub fn encode(
        (start, end): (Position, Position),
        encoding: PositionEncoding,
        doc: &Doc,
    ) -> LspRange {
        let start = LspPosition::encode(start, encoding, doc);
        let end = LspPosition::encode(end, encoding, doc);

        LspRange { start, end }
    }

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LspTextEdit {
    range: LspRange,
    new_text: String,
}

impl LspTextEdit {
    fn decode(self, encoding: PositionEncoding, doc: &Doc) -> TextEdit {
        TextEdit {
            range: self.range.decode(encoding, doc),
            new_text: self.new_text,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LspCompletionItem {
    label: String,
    sort_text: Option<String>,
    filter_text: Option<String>,
    insert_text: Option<String>,
    text_edit: Option<LspTextEdit>,
    #[serde(default)]
    additional_text_edits: Vec<LspTextEdit>,
    detail: Option<String>,
    documentation: Option<Documentation>,
    data: Option<Value>,
}

impl LspCompletionItem {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> CompletionItem {
        CompletionItem {
            label: self.label,
            sort_text: self.sort_text,
            filter_text: self.filter_text,
            insert_text: self.insert_text,
            text_edit: self
                .text_edit
                .map(|text_edit| text_edit.decode(encoding, doc)),
            additional_text_edits: self
                .additional_text_edits
                .into_iter()
                .map(|text_edit| text_edit.decode(encoding, doc))
                .collect(),
            detail: self.detail,
            documentation: self.documentation,
            data: self.data,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct LspCompletionList {
    pub items: Vec<LspCompletionItem>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum LspCompletionResult {
    List(LspCompletionList),
    Items(Vec<LspCompletionItem>),
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
    pub id: Option<usize>,
    pub method: Option<String>,
    pub result: Option<Box<RawValue>>,
    pub params: Option<Box<RawValue>>,
}

#[derive(Debug, Deserialize)]
struct LspTextDocumentIdentifier<'a> {
    uri: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LspTextDocumentEdit<'a> {
    #[serde(borrow)]
    text_document: LspTextDocumentIdentifier<'a>,
    edits: Vec<LspTextEdit>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LspWorkspaceEdit<'a> {
    #[serde(borrow)]
    changes: Option<HashMap<&'a str, Vec<LspTextEdit>>>,
    document_changes: Option<Vec<LspTextDocumentEdit<'a>>>,
}

impl LspWorkspaceEdit<'_> {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> Vec<EditList> {
        let mut edit = Vec::new();

        if let Some(changes) = self.document_changes {
            for change in changes {
                let edits = change
                    .edits
                    .into_iter()
                    .map(|text_edit| text_edit.decode(encoding, doc))
                    .collect();

                edit.push(EditList {
                    uri: change.text_document.uri.to_string(),
                    edits,
                });
            }
        } else if let Some(changes) = self.changes {
            for (uri, edits) in changes {
                let edits = edits
                    .into_iter()
                    .map(|text_edit| text_edit.decode(encoding, doc))
                    .collect();

                edit.push(EditList {
                    uri: uri.to_string(),
                    edits,
                });
            }
        }

        edit
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
pub(super) enum LspPrepareRenameResult {
    Range(LspRange),
    RangeWithPlaceholder {
        range: LspRange,
        placeholder: String,
    },
    #[default]
    Invalid,
}

#[derive(Debug, Deserialize)]
pub struct Command {
    pub title: String,
    pub command: String,
    #[serde(default)]
    pub arguments: Vec<Box<RawValue>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LspCodeAction<'a> {
    title: String,
    #[serde(borrow)]
    edit: Option<LspWorkspaceEdit<'a>>,
    command: Option<Command>,
    #[serde(default)]
    is_preferred: bool,
}

impl LspCodeAction<'_> {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> CodeAction {
        let edit_lists = self
            .edit
            .map(|edit| edit.decode(encoding, doc))
            .unwrap_or_default();

        CodeAction {
            title: self.title,
            edit_lists,
            command: self.command,
            is_preferred: self.is_preferred,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum LspCodeActionResult<'a> {
    Command(Command),
    #[serde(borrow)]
    CodeAction(LspCodeAction<'a>),
    None,
}

impl LspCodeActionResult<'_> {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> CodeActionResult {
        match self {
            LspCodeActionResult::Command(command) => CodeActionResult::Command(command),
            LspCodeActionResult::CodeAction(code_action) => {
                CodeActionResult::CodeAction(code_action.decode(encoding, doc))
            }
            LspCodeActionResult::None => panic!(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub range: (Position, Position),
    pub severity: usize,
}

impl Diagnostic {
    pub fn is_problem(&self) -> bool {
        self.severity <= 2
    }

    pub fn color(&self, theme: &Theme) -> Color {
        match self.severity {
            1 => theme.error,
            2 => theme.warning,
            _ => theme.info,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextEdit {
    pub range: (Position, Position),
    pub new_text: String,
}

impl TextEdit {
    fn encode(self, encoding: PositionEncoding, doc: &Doc) -> LspTextEdit {
        LspTextEdit {
            range: LspRange::encode(self.range, encoding, doc),
            new_text: self.new_text,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Documentation {
    PlainText(String),
    MarkupContent { kind: String, value: String },
}

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    sort_text: Option<String>,
    filter_text: Option<String>,
    pub insert_text: Option<String>,
    pub text_edit: Option<TextEdit>,
    pub additional_text_edits: Vec<TextEdit>,
    pub detail: Option<String>,
    pub documentation: Option<Documentation>,
    data: Option<Value>,
}

impl CompletionItem {
    pub fn sort_text(&self) -> &str {
        self.sort_text.as_ref().unwrap_or(&self.label)
    }

    pub fn filter_text(&self) -> &str {
        self.filter_text.as_ref().unwrap_or(&self.label)
    }

    pub fn insert_text(&self) -> &str {
        self.text_edit
            .as_ref()
            .map(|text_edit| &text_edit.new_text)
            .or(self.insert_text.as_ref())
            .unwrap_or(&self.label)
    }

    pub fn range(&self) -> Option<(Position, Position)> {
        self.text_edit.as_ref().map(|text_edit| text_edit.range)
    }

    pub(super) fn encode(self, encoding: PositionEncoding, doc: &Doc) -> LspCompletionItem {
        LspCompletionItem {
            label: self.label,
            sort_text: self.sort_text,
            filter_text: self.filter_text,
            insert_text: self.insert_text,
            text_edit: self
                .text_edit
                .map(|text_edit| text_edit.encode(encoding, doc)),
            additional_text_edits: self
                .additional_text_edits
                .into_iter()
                .map(|text_edit| text_edit.encode(encoding, doc))
                .collect(),
            detail: self.detail,
            documentation: self.documentation,
            data: self.data,
        }
    }
}

#[derive(Debug)]
pub struct EditList {
    pub uri: String,
    pub edits: Vec<TextEdit>,
}

#[derive(Debug)]
pub struct CodeAction {
    pub title: String,
    pub edit_lists: Vec<EditList>,
    pub command: Option<Command>,
    pub is_preferred: bool,
}

#[derive(Debug)]
pub enum CodeActionResult {
    Command(Command),
    CodeAction(CodeAction),
}
