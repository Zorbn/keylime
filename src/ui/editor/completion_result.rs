use serde_json::value::RawValue;

use crate::{
    geometry::position::Position,
    lsp::types::{CodeAction, CodeActionDocumentEdit, Command, CompletionItem, TextEdit},
    text::line_pool::LinePool,
};

#[derive(Debug)]
pub struct CompletionCommand {
    pub command: String,
    pub arguments: Vec<Box<RawValue>>,
}

impl CompletionCommand {
    pub fn from_command(command: Command, pool: &mut LinePool) -> Self {
        let mut command_string = pool.pop();
        command_string.push_str(command.command);

        Self {
            command: command_string,
            arguments: command.arguments,
        }
    }
}

#[derive(Debug)]
pub struct CompletionTextEdit {
    pub range: (Position, Position),
    pub new_text: String,
}

impl CompletionTextEdit {
    pub fn from_text_edit(edit: TextEdit, pool: &mut LinePool) -> Self {
        let mut new_text = pool.pop();
        new_text.push_str(&edit.new_text);

        Self {
            range: edit.range,
            new_text,
        }
    }
}

#[derive(Debug)]
pub struct CompletionDocumentTextEdit {
    pub uri: String,
    pub edits: Vec<CompletionTextEdit>,
}

impl CompletionDocumentTextEdit {
    pub fn from_code_action_document_edit(
        edit: CodeActionDocumentEdit,
        pool: &mut LinePool,
    ) -> Self {
        let edits = edit
            .edits
            .into_iter()
            .map(|edit| CompletionTextEdit::from_text_edit(edit, pool))
            .collect();

        let mut uri_string = pool.pop();
        uri_string.push_str(edit.uri);

        Self {
            uri: uri_string,
            edits,
        }
    }
}

pub enum CompletionResultAction {
    Completion {
        insert_text: Option<String>,
        range: Option<(Position, Position)>,
    },
    Command(CompletionCommand),
    CodeAction {
        edits: Vec<CompletionDocumentTextEdit>,
        command: Option<CompletionCommand>,
    },
}

pub struct CompletionResult {
    pub label: String,
    pub action: CompletionResultAction,
}

impl CompletionResult {
    pub fn push_to_pool(self, pool: &mut LinePool) {
        pool.push(self.label);

        match self.action {
            CompletionResultAction::Completion { insert_text, .. } => {
                if let Some(insert_text) = insert_text {
                    pool.push(insert_text);
                }
            }
            CompletionResultAction::Command(command) => pool.push(command.command),
            CompletionResultAction::CodeAction { edits, command } => {
                for edit in edits {
                    pool.push(edit.uri);

                    for edit in edit.edits {
                        pool.push(edit.new_text);
                    }
                }

                if let Some(command) = command {
                    pool.push(command.command);
                }
            }
        }
    }

    pub fn from_completion_item(item: CompletionItem, pool: &mut LinePool) -> Self {
        let (label, insert_text, range) = if let Some(text_edit) = &item.text_edit {
            (
                item.label,
                Some(text_edit.new_text.clone()),
                Some(text_edit.range),
            )
        } else {
            (item.label, item.insert_text, None)
        };

        let mut label_string = pool.pop();
        label_string.push_str(label);

        let insert_text_string = insert_text.map(|insert_text| {
            let mut insert_text_string = pool.pop();
            insert_text_string.push_str(&insert_text);
            insert_text_string
        });

        Self {
            label: label_string,
            action: CompletionResultAction::Completion {
                insert_text: insert_text_string,
                range,
            },
        }
    }

    pub fn from_command(command: Command, pool: &mut LinePool) -> Self {
        let mut label = pool.pop();
        label.push_str(command.title);

        let command = CompletionCommand::from_command(command, pool);

        Self {
            label,
            action: CompletionResultAction::Command(command),
        }
    }

    pub fn from_code_action(code_action: CodeAction, pool: &mut LinePool) -> (Self, bool) {
        let mut label = pool.pop();
        label.push_str(code_action.title);

        let command = code_action
            .command
            .map(|command| CompletionCommand::from_command(command, pool));

        let completion_edits = code_action
            .edit
            .into_iter()
            .map(|edit| CompletionDocumentTextEdit::from_code_action_document_edit(edit, pool))
            .collect();

        (
            Self {
                label,
                action: CompletionResultAction::CodeAction {
                    edits: completion_edits,
                    command,
                },
            },
            code_action.is_preferred,
        )
    }
}
