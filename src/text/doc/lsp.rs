use std::collections::HashMap;

use crate::{
    ctx::Ctx,
    geometry::position::Position,
    lsp::{
        language_server::LanguageServer,
        types::{DecodedRange, DecodedTextEdit},
        LspExpectedResponse, LspSentRequest,
    },
    pool::STRING_POOL,
    text::{
        diff::{myers_diff, DiffEdit},
        selection::Selection,
    },
};

use crate::text::cursor_index::CursorIndex;

use super::{Doc, DocFlag};

#[derive(Debug, Default)]
pub(super) struct DocLspState {
    expected_responses: HashMap<&'static str, LspExpectedResponse>,
    is_open: bool,
    debounced_requests: HashMap<&'static str, Option<Position>>,
}

impl Doc {
    pub fn get_language_server_mut<'a>(&self, ctx: &'a mut Ctx) -> Option<&'a mut LanguageServer> {
        if !self.flags.contains(DocFlag::AllowLanguageServer) {
            return None;
        }

        ctx.lsp
            .get_language_server_mut(self, ctx.config, ctx.current_dir)
    }

    fn lsp_add_expected_response(
        &mut self,
        sent_request: LspSentRequest,
        position: Option<Position>,
    ) {
        self.lsp_state.expected_responses.insert(
            sent_request.method,
            LspExpectedResponse {
                id: sent_request.id,
                position,
                version: self.version,
            },
        );
    }

    pub fn lsp_is_response_expected(
        &mut self,
        method: &str,
        id: Option<usize>,
        ctx: &mut Ctx,
    ) -> bool {
        let Some(id) = id else {
            // This was a notification so it's expected by default.
            return true;
        };

        let Some(expected_response) = self.lsp_state.expected_responses.get(method).copied() else {
            // Expected responses don't need to be tracked for this method.
            return true;
        };

        if expected_response.id != id {
            return false;
        }

        self.lsp_state.expected_responses.remove(method);

        if self.lsp_debounced_request(method) {
            match method {
                "textDocument/completion" => self.lsp_completion(ctx),
                "textDocument/diagnostic" => self.lsp_diagnostic(ctx),
                _ => None,
            };
        }

        let position = self.cursor(CursorIndex::Main).position;

        let is_position_expected = expected_response
            .position
            .is_none_or(|expected_position| expected_position == position);

        let is_version_expected = expected_response.version == self.version;

        if !is_position_expected || !is_version_expected {
            // We received the expected response, but the doc didn't match the expected state.
            return false;
        }

        true
    }

    pub fn lsp_did_open(&mut self, text: &str, ctx: &mut Ctx) -> Option<()> {
        if self.lsp_state.is_open {
            return None;
        }

        let language = ctx.config.get_language_for_doc(self)?;
        let language_server = self.get_language_server_mut(ctx)?;
        let language_id = language.lsp.language_id.as_ref()?;
        let path = self.path.some()?;

        language_server.did_open(path, language_id, self.version, text);
        self.lsp_diagnostic(ctx);

        self.lsp_state.is_open = true;

        Some(())
    }

    pub fn lsp_did_close(&mut self, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_state.is_open {
            return None;
        }

        self.lsp_text_document_notification("textDocument/didClose", ctx)?;

        self.lsp_state.is_open = false;

        Some(())
    }

    pub fn lsp_did_change(
        &self,
        start: Position,
        end: Position,
        text: &str,
        ctx: &mut Ctx,
    ) -> Option<()> {
        if !self.lsp_state.is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        language_server.did_change(path, self.version, start, end, text, self);

        Some(())
    }

    pub fn lsp_diagnostic(&mut self, ctx: &mut Ctx) -> Option<()> {
        if self.lsp_debounce_request("textDocument/diagnostic", None) {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        let sent_request = language_server.diagnostic(path)?;
        self.lsp_add_expected_response(sent_request, None);

        Some(())
    }

    pub fn lsp_completion(&mut self, ctx: &mut Ctx) -> Option<()> {
        let position = self.cursor(CursorIndex::Main).position;

        if self.lsp_debounce_request("textDocument/completion", Some(position)) {
            return None;
        }

        self.get_completion_prefix(ctx.gfx)?;

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        let sent_request = language_server.completion(path, position, self);
        self.lsp_add_expected_response(sent_request, Some(position));

        Some(())
    }

    pub fn lsp_code_action(&mut self, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_state.is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        let cursor = self.cursor(CursorIndex::Main);

        let (start, end) = if let Some(selection) = cursor.get_selection() {
            (selection.start, selection.end)
        } else {
            (cursor.position, cursor.position)
        };

        let sent_request = language_server.code_action(path, start, end, self);
        self.lsp_add_expected_response(sent_request, Some(cursor.position));

        Some(())
    }

    pub fn lsp_prepare_rename(&mut self, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_state.is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;
        let position = self.cursor(CursorIndex::Main).position;

        let sent_request = language_server.prepare_rename(path, position, self);
        self.lsp_add_expected_response(sent_request, Some(position));

        Some(())
    }

    pub fn lsp_rename(&self, new_name: &str, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_state.is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;
        let position = self.cursor(CursorIndex::Main).position;

        language_server.rename(new_name, path, position, self);

        Some(())
    }

    pub fn lsp_references(&mut self, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_state.is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;
        let position = self.cursor(CursorIndex::Main).position;

        let sent_request = language_server.references(path, position, self);
        self.lsp_add_expected_response(sent_request, Some(position));

        Some(())
    }

    pub fn lsp_definition(&mut self, position: Position, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_state.is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        let sent_request = language_server.definition(path, position, self);
        self.lsp_add_expected_response(sent_request, None);

        Some(())
    }

    pub fn lsp_signature_help(
        &mut self,
        trigger_char: Option<char>,
        is_retrigger: bool,
        ctx: &mut Ctx,
    ) -> Option<()> {
        if !self.lsp_state.is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;
        let position = self.cursor(CursorIndex::Main).position;

        let sent_request =
            language_server.signature_help(path, position, trigger_char, is_retrigger, self);
        self.lsp_add_expected_response(sent_request, Some(position));

        Some(())
    }

    pub fn lsp_hover(&mut self, position: Position, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_state.is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        let sent_request = language_server.hover(path, position, self);
        self.lsp_add_expected_response(sent_request, None);

        Some(())
    }

    pub fn lsp_formatting(&self, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_state.is_open {
            return None;
        }

        let indent_width = ctx.config.indent_width_for_doc(self);

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        language_server.formatting(path, indent_width);

        Some(())
    }

    pub fn lsp_text_document_notification(
        &self,
        method: &'static str,
        ctx: &mut Ctx,
    ) -> Option<()> {
        if !self.lsp_state.is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        language_server.text_document_notification(path, method);

        Some(())
    }

    pub fn lsp_apply_edit_list(&mut self, edits: &mut [DecodedTextEdit], ctx: &mut Ctx) {
        let mut tmp = self.tmp_clone();

        // let needs_skip_shifting = edits.iter().any(|edit| {
        //     println!(
        //         "shift check: got ({:?}, {:?}), checked for ({:?}, {:?})",
        //         edit.range.start,
        //         edit.range.end,
        //         Position::ZERO,
        //         self.end()
        //     );
        //     edit.range.start == Position::ZERO && edit.range.end == self.end()
        // });

        // if needs_skip_shifting {
        //     self.start_skipping_shifting(ctx.time);
        // }

        for i in 0..edits.len() {
            let current_edit = &edits[i];

            let DecodedRange { start, end } = current_edit.range;

            tmp.delete(start, end, ctx);
            let insert_end = tmp.insert(start, &current_edit.new_text, ctx);

            for DecodedTextEdit { range, .. } in edits.iter_mut().skip(i + 1) {
                range.start = tmp.shift_position_by_delete(start, end, range.start);
                range.end = tmp.shift_position_by_delete(start, end, range.end);

                range.start = tmp.shift_position_by_insert(start, insert_end, range.start);
                range.end = tmp.shift_position_by_insert(start, insert_end, range.end);
            }
        }

        // TODO: Pull this out into a Doc::apply_diff function in doc.rs!
        let edits = myers_diff(self.lines(), &tmp.lines);

        // TODO: This was copied from handle_cut.
        fn select_line(doc: &Doc, position: Position, ctx: &mut Ctx) -> Selection {
            let mut selection = doc.select_current_line_at_position(position, ctx.gfx);

            if position.y == doc.lines().len() - 1 {
                selection.start = doc.move_position(selection.start, -1, 0, ctx.gfx);
            }

            selection
        }

        let mut buffer = STRING_POOL.new_item();
        let mut a_index = 0;

        for edit in edits {
            match edit {
                DiffEdit::Delete => {
                    // result.remove(a_index);
                    let selection = select_line(self, Position::new(0, a_index), ctx);
                    self.delete(selection.start, selection.end, ctx);
                }
                DiffEdit::Insert { b_index } => {
                    // result.replace_range(a_index..a_index, &b[*b_index..*b_index + 1]);
                    let position = Position::new(0, a_index);
                    let selection = select_line(&tmp, Position::new(0, b_index), ctx);
                    tmp.collect_string(selection.start, selection.end, &mut buffer);
                    self.insert(position, &buffer, ctx);

                    a_index += 1;
                }
                DiffEdit::Match => {
                    a_index += 1;
                }
                DiffEdit::Substite { b_index } => {
                    // result.replace_range(a_index..a_index + 1, &b[*b_index..*b_index + 1]);

                    // let position = Position::new(0, a_index);
                    // let selection = select_line(self, position, ctx);
                    // self.delete(selection.start, selection.end, ctx);

                    // let selection = select_line(&tmp, Position::new(0, b_index), ctx);
                    // tmp.collect_string(selection.start, selection.end, &mut buffer);
                    // self.insert(position, &buffer, ctx);

                    let a = &self.lines[a_index].as_bytes();
                    let b = &tmp.lines[b_index].as_bytes();
                    let edits = myers_diff(a, b);

                    let y = a_index;

                    {
                        let mut a_index = 0;

                        // TODO: We need to combine edits, imagine you insert an emoji. It's only valid as a multi-byte edit.
                        for edit in edits {
                            match edit {
                                DiffEdit::Delete => {
                                    // result.remove(a_index);

                                    self.delete(
                                        Position::new(a_index, y),
                                        Position::new(a_index + 1, y),
                                        ctx,
                                    );
                                }
                                DiffEdit::Insert { b_index } => {
                                    // result.replace_range(a_index..a_index, &b[*b_index..*b_index + 1]);

                                    self.insert(
                                        Position::new(a_index, y),
                                        // TODO: No unwrap here, it could actually not be utf8 if the LSP has a bug.
                                        str::from_utf8(&b[b_index..b_index + 1]).unwrap(),
                                        ctx,
                                    );

                                    a_index += 1;
                                }
                                DiffEdit::Match => {
                                    a_index += 1;
                                }
                                DiffEdit::Substite { b_index } => {
                                    // result.replace_range(
                                    //     a_index..a_index + 1,
                                    //     &b[*b_index..*b_index + 1],
                                    // );

                                    self.delete(
                                        Position::new(a_index, y),
                                        Position::new(a_index + 1, y),
                                        ctx,
                                    );

                                    self.insert(
                                        Position::new(a_index, y),
                                        // TODO: No unwrap here, it could actually not be utf8 if the LSP has a bug.
                                        str::from_utf8(&b[b_index..b_index + 1]).unwrap(),
                                        ctx,
                                    );

                                    a_index += 1;
                                }
                            }
                        }
                    }

                    a_index += 1;
                }
            }
        }

        // if needs_skip_shifting {
        //     self.stop_skipping_shifting(ctx);
        // }
    }

    pub fn lsp_debounce_request(
        &mut self,
        method: &'static str,
        position: Option<Position>,
    ) -> bool {
        if !self.lsp_state.expected_responses.contains_key(method) {
            return false;
        }

        self.lsp_state.debounced_requests.insert(method, position);
        true
    }

    pub fn lsp_debounced_request(&mut self, method: &str) -> bool {
        let position = self.cursor(CursorIndex::Main).position;

        self.lsp_state
            .debounced_requests
            .remove(method)
            .filter(|dp| dp.is_none_or(|dp| dp == position))
            .is_some()
    }
}
