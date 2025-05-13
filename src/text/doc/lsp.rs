use crate::{
    ctx::Ctx,
    geometry::position::Position,
    lsp::{language_server::LanguageServer, LspExpectedResponse, LspSentRequest},
};

use crate::text::cursor_index::CursorIndex;

use super::{Doc, DocKind};

impl Doc {
    pub fn get_language_server_mut<'a>(&self, ctx: &'a mut Ctx) -> Option<&'a mut LanguageServer> {
        if self.kind == DocKind::Output {
            return None;
        }

        ctx.lsp.get_language_server_mut(self, ctx.config)
    }

    fn lsp_add_expected_response(
        &mut self,
        sent_request: LspSentRequest,
        position: Option<Position>,
    ) {
        self.lsp_expected_responses.insert(
            sent_request.method,
            LspExpectedResponse {
                id: sent_request.id,
                position,
                version: self.version,
            },
        );
    }

    pub fn lsp_is_response_expected(&mut self, method: &str, id: Option<usize>) -> bool {
        let Some(id) = id else {
            // This was a notification so it's expected by default.
            return true;
        };

        let Some(expected_response) = self.lsp_expected_responses.get(method).copied() else {
            // Expected responses don't need to be tracked for this method.
            return true;
        };

        if expected_response.id != id {
            return false;
        }

        self.lsp_expected_responses.remove(method);

        let position = self.get_cursor(CursorIndex::Main).position;

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
        if self.lsp_is_open {
            return None;
        }

        let language = ctx.config.get_language_for_doc(self)?;
        let language_server = self.get_language_server_mut(ctx)?;
        let language_id = language.lsp_language_id.as_ref()?;
        let path = self.path.some()?;

        language_server.did_open(path, language_id, self.version, text);
        self.lsp_diagnostic(ctx);

        self.lsp_is_open = true;

        Some(())
    }

    pub fn lsp_did_close(&mut self, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_is_open {
            return None;
        }

        self.lsp_text_document_notification("textDocument/didClose", ctx)?;

        self.lsp_is_open = false;

        Some(())
    }

    pub fn lsp_did_change(
        &mut self,
        start: Position,
        end: Position,
        text: &str,
        ctx: &mut Ctx,
    ) -> Option<()> {
        if !self.lsp_is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        language_server.did_change(path, self.version, start, end, text, self);

        Some(())
    }

    pub fn lsp_diagnostic(&mut self, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_is_open
            || self
                .lsp_expected_responses
                .contains_key("textDocument/diagnostic")
        {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        let sent_request = language_server.diagnostic(path)?;
        self.lsp_add_expected_response(sent_request, None);

        Some(())
    }

    pub fn lsp_completion(&mut self, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_is_open
            || self
                .lsp_expected_responses
                .contains_key("textDocument/completion")
        {
            return None;
        }

        self.get_completion_prefix(ctx.gfx)?;

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;
        let position = self.get_cursor(CursorIndex::Main).position;

        let sent_request = language_server.completion(path, position, self);
        self.lsp_add_expected_response(sent_request, Some(position));

        Some(())
    }

    pub fn lsp_code_action(&mut self, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        let cursor = self.get_cursor(CursorIndex::Main);

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
        if !self.lsp_is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;
        let position = self.get_cursor(CursorIndex::Main).position;

        let sent_request = language_server.prepare_rename(path, position, self);
        self.lsp_add_expected_response(sent_request, Some(position));

        Some(())
    }

    pub fn lsp_rename(&self, new_name: &str, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;
        let position = self.get_cursor(CursorIndex::Main).position;

        language_server.rename(new_name, path, position, self);

        Some(())
    }

    pub fn lsp_references(&mut self, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;
        let position = self.get_cursor(CursorIndex::Main).position;

        let sent_request = language_server.references(path, position, self);
        self.lsp_add_expected_response(sent_request, Some(position));

        Some(())
    }

    pub fn lsp_definition(&mut self, position: Position, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_is_open {
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
        if !self.lsp_is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;
        let position = self.get_cursor(CursorIndex::Main).position;

        let sent_request =
            language_server.signature_help(path, position, trigger_char, is_retrigger, self);
        self.lsp_add_expected_response(sent_request, Some(position));

        Some(())
    }

    pub fn lsp_formatting(&mut self, ctx: &mut Ctx) -> Option<()> {
        if !self.lsp_is_open {
            return None;
        }

        let indent_width = ctx.config.get_indent_width_for_doc(self);

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        language_server.formatting(path, indent_width);

        Some(())
    }

    pub fn lsp_text_document_notification(
        &mut self,
        method: &'static str,
        ctx: &mut Ctx,
    ) -> Option<()> {
        if !self.lsp_is_open {
            return None;
        }

        let language_server = self.get_language_server_mut(ctx)?;
        let path = self.path.some()?;

        language_server.text_document_notification(path, method);

        Some(())
    }
}
