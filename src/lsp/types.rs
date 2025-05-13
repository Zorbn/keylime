use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{value::RawValue, Value};

use crate::{
    config::theme::Theme, geometry::position::Position, pool::Pooled, text::doc::Doc,
    ui::color::Color,
};

use super::position_encoding::PositionEncoding;

const DEFAULT_SEVERITY: fn() -> usize = || 1;

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub(super) struct EncodedPosition {
    line: usize,
    character: usize,
}

impl EncodedPosition {
    pub fn encode(position: Position, encoding: PositionEncoding, doc: &Doc) -> EncodedPosition {
        let line = doc.get_line(position.y).unwrap_or_default();

        match encoding {
            PositionEncoding::Utf8 => EncodedPosition {
                line: position.y,
                character: position.x,
            },
            PositionEncoding::Utf16 => EncodedPosition {
                line: position.y,
                character: line[..position.x].encode_utf16().count(),
            },
        }
    }

    fn decode(self, encoding: PositionEncoding, doc: &Doc) -> Position {
        match encoding {
            PositionEncoding::Utf8 => Position {
                x: self.character,
                y: self.line,
            },
            PositionEncoding::Utf16 => {
                let line = doc.get_line(self.line).unwrap_or_default();

                let mut wide_index = 0;
                let mut result = Position {
                    x: line.len(),
                    y: self.line,
                };

                for (index, c) in line.char_indices() {
                    let mut dst = [0; 2];

                    wide_index += c.encode_utf16(&mut dst).len();

                    if wide_index > self.character {
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
pub(super) struct EncodedRange {
    start: EncodedPosition,
    end: EncodedPosition,
}

impl EncodedRange {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> DecodedRange {
        DecodedRange {
            start: self.start.decode(encoding, doc),
            end: self.end.decode(encoding, doc),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct EncodedDiagnostic {
    message: Pooled<String>,
    range: EncodedRange,
    #[serde(default = "DEFAULT_SEVERITY")]
    pub severity: usize,
}

impl EncodedDiagnostic {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> DecodedDiagnostic {
        DecodedDiagnostic {
            message: self.message,
            range: self.range.decode(encoding, doc),
            severity: self.severity,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct EncodedPublishDiagnosticsParams {
    pub uri: Pooled<String>,
    pub diagnostics: Vec<EncodedDiagnostic>,
}

#[derive(Debug, Deserialize)]
pub(super) struct EncodedFullDocumentDiagnosticParams {
    pub items: Vec<EncodedDiagnostic>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EncodedTextEdit {
    range: EncodedRange,
    new_text: Pooled<String>,
}

impl EncodedTextEdit {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> DecodedTextEdit {
        DecodedTextEdit {
            range: self.range.decode(encoding, doc),
            new_text: self.new_text,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EncodedCompletionItem {
    label: Pooled<String>,
    sort_text: Option<Pooled<String>>,
    filter_text: Option<Pooled<String>>,
    insert_text: Option<Pooled<String>>,
    text_edit: Option<EncodedTextEdit>,
    #[serde(default)]
    additional_text_edits: Vec<EncodedTextEdit>,
    detail: Option<Pooled<String>>,
    documentation: Option<Documentation>,
    data: Option<Value>,
}

impl EncodedCompletionItem {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> DecodedCompletionItem {
        DecodedCompletionItem {
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
pub(super) struct EncodedCompletionList {
    pub items: Vec<EncodedCompletionItem>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum LspCompletionResult {
    List(EncodedCompletionList),
    Items(Vec<EncodedCompletionItem>),
    None,
}

#[derive(Debug, Deserialize)]
pub(super) struct EncodedLocation<'a> {
    pub uri: &'a str,
    pub range: EncodedRange,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EncodedLocationLink<'a> {
    pub target_uri: &'a str,
    pub target_range: EncodedRange,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum EncodedDefinitionResult<'a> {
    #[serde(borrow)]
    Location(EncodedLocation<'a>),
    Locations(Vec<EncodedLocation<'a>>),
    Links(Vec<EncodedLocationLink<'a>>),
    None,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SignatureHelpOptions {
    #[serde(default)]
    pub trigger_characters: Vec<Pooled<String>>,
    #[serde(default)]
    pub retrigger_characters: Vec<Pooled<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CompletionOptions {
    #[serde(default)]
    pub resolve_provider: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ServerCapabilities<'a> {
    pub position_encoding: &'a str,
    pub signature_help_provider: Option<SignatureHelpOptions>,
    pub diagnostic_provider: Option<()>,
    pub completion_provider: Option<CompletionOptions>,
}

#[derive(Debug, Deserialize)]
pub(super) struct InitializeResult<'a> {
    #[serde(borrow)]
    pub capabilities: ServerCapabilities<'a>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Registration<'a> {
    pub method: &'a str,
}

#[derive(Debug, Deserialize)]
pub(super) struct RegistrationParams<'a> {
    #[serde(borrow)]
    pub registrations: Vec<Registration<'a>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Message {
    pub id: Option<usize>,
    pub method: Option<Pooled<String>>,
    pub result: Option<Box<RawValue>>,
    pub params: Option<Box<RawValue>>,
}

#[derive(Debug, Deserialize)]
struct TextDocumentIdentifier {
    uri: Pooled<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EncodedTextDocumentEdit {
    text_document: TextDocumentIdentifier,
    edits: Vec<EncodedTextEdit>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EncodedWorkspaceEdit {
    changes: Option<HashMap<Pooled<String>, Vec<EncodedTextEdit>>>,
    document_changes: Option<Vec<EncodedTextDocumentEdit>>,
}

impl EncodedWorkspaceEdit {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> Vec<DecodedEditList> {
        let mut edit = Vec::new();

        if let Some(changes) = self.document_changes {
            for change in changes {
                let edits = change
                    .edits
                    .into_iter()
                    .map(|text_edit| text_edit.decode(encoding, doc))
                    .collect();

                edit.push(DecodedEditList {
                    uri: change.text_document.uri,
                    edits,
                });
            }
        } else if let Some(changes) = self.changes {
            for (uri, edits) in changes {
                let edits = edits
                    .into_iter()
                    .map(|text_edit| text_edit.decode(encoding, doc))
                    .collect();

                edit.push(DecodedEditList { uri, edits });
            }
        }

        edit
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
pub(super) enum LspPrepareRenameResult {
    Range(EncodedRange),
    RangeWithPlaceholder {
        range: EncodedRange,
        placeholder: Pooled<String>,
    },
    #[default]
    Invalid,
}

#[derive(Debug, Deserialize)]
pub struct Command {
    pub title: Pooled<String>,
    pub command: Pooled<String>,
    #[serde(default)]
    pub arguments: Vec<Box<RawValue>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EncodedCodeAction {
    title: Pooled<String>,
    edit: Option<EncodedWorkspaceEdit>,
    command: Option<Command>,
    #[serde(default)]
    is_preferred: bool,
}

impl EncodedCodeAction {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> DecodedCodeAction {
        let edit_lists = self
            .edit
            .map(|edit| edit.decode(encoding, doc))
            .unwrap_or_default();

        DecodedCodeAction {
            title: self.title,
            edit_lists,
            command: self.command,
            is_preferred: self.is_preferred,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum LspCodeActionResult {
    Command(Command),
    CodeAction(EncodedCodeAction),
    None,
}

impl LspCodeActionResult {
    pub fn decode(self, encoding: PositionEncoding, doc: &Doc) -> DecodedCodeActionResult {
        match self {
            LspCodeActionResult::Command(command) => DecodedCodeActionResult::Command(command),
            LspCodeActionResult::CodeAction(code_action) => {
                DecodedCodeActionResult::CodeAction(code_action.decode(encoding, doc))
            }
            LspCodeActionResult::None => panic!(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DecodedRange {
    pub start: Position,
    pub end: Position,
}

impl DecodedRange {
    pub(super) fn encode(self, encoding: PositionEncoding, doc: &Doc) -> EncodedRange {
        EncodedRange {
            start: EncodedPosition::encode(self.start, encoding, doc),
            end: EncodedPosition::encode(self.end, encoding, doc),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodedDiagnostic {
    pub message: Pooled<String>,
    pub range: DecodedRange,
    pub severity: usize,
}

impl DecodedDiagnostic {
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

    pub fn visible_range(&self, doc: &Doc) -> DecodedRange {
        let DecodedRange { mut start, mut end } = self.range;

        start.x = start.x.max(doc.line_start(start.y));
        end.x = end.x.max(doc.line_start(end.y));

        DecodedRange { start, end }
    }

    pub fn contains_position(&self, position: Position, doc: &Doc) -> bool {
        let DecodedRange { start, end } = self.range;

        position.x >= doc.line_start(position.y) && position >= start && position <= end
    }

    pub(super) fn encode(&self, encoding: PositionEncoding, doc: &Doc) -> EncodedDiagnostic {
        EncodedDiagnostic {
            message: self.message.clone(),
            range: self.range.encode(encoding, doc),
            severity: self.severity,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodedTextEdit {
    pub range: DecodedRange,
    pub new_text: Pooled<String>,
}

impl DecodedTextEdit {
    fn encode(self, encoding: PositionEncoding, doc: &Doc) -> EncodedTextEdit {
        EncodedTextEdit {
            range: self.range.encode(encoding, doc),
            new_text: self.new_text,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Documentation {
    PlainText(Pooled<String>),
    MarkupContent {
        kind: Pooled<String>,
        value: Pooled<String>,
    },
}

impl Documentation {
    pub fn text(&self) -> &str {
        match self {
            Documentation::PlainText(text) => text,
            Documentation::MarkupContent { value, .. } => value,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodedCompletionItem {
    pub label: Pooled<String>,
    sort_text: Option<Pooled<String>>,
    filter_text: Option<Pooled<String>>,
    pub insert_text: Option<Pooled<String>>,
    pub text_edit: Option<DecodedTextEdit>,
    pub additional_text_edits: Vec<DecodedTextEdit>,
    pub detail: Option<Pooled<String>>,
    pub documentation: Option<Documentation>,
    data: Option<Value>,
}

impl DecodedCompletionItem {
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

    pub fn range(&self) -> Option<DecodedRange> {
        self.text_edit.as_ref().map(|text_edit| text_edit.range)
    }

    pub(super) fn encode(self, encoding: PositionEncoding, doc: &Doc) -> EncodedCompletionItem {
        EncodedCompletionItem {
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
pub struct DecodedEditList {
    pub uri: Pooled<String>,
    pub edits: Vec<DecodedTextEdit>,
}

#[derive(Debug)]
pub struct DecodedCodeAction {
    pub title: Pooled<String>,
    pub edit_lists: Vec<DecodedEditList>,
    pub command: Option<Command>,
    pub is_preferred: bool,
}

#[derive(Debug)]
pub enum DecodedCodeActionResult {
    Command(Command),
    CodeAction(DecodedCodeAction),
}

#[derive(Debug, Deserialize)]
pub struct SignatureInformation {
    pub label: Pooled<String>,
    pub documentation: Option<Documentation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    #[serde(default)]
    pub active_signature: usize,
}
