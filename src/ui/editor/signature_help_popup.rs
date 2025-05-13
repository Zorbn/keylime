use std::path::PathBuf;

use crate::{
    ctx::Ctx,
    geometry::position::Position,
    lsp::types::SignatureHelp,
    text::{cursor_index::CursorIndex, doc::Doc},
    ui::{
        core::{Ui, Widget},
        popup::{draw_popup, PopupAlignment},
        tab::Tab,
    },
};

pub struct SignatureHelpPopup {
    help_path: PathBuf,
    help_position: Position,
    help: Option<SignatureHelp>,
}

impl SignatureHelpPopup {
    pub fn new() -> Self {
        Self {
            help_path: PathBuf::new(),
            help_position: Position::ZERO,
            help: None,
        }
    }

    pub fn update(
        &mut self,
        (trigger_char, retrigger_char): (Option<char>, Option<char>),
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) {
        if Some(self.help_path.as_path()) != doc.path().some_path() {
            self.clear();
        }

        let position = doc.cursor(CursorIndex::Main).position;
        let is_retrigger = self.help.is_some();

        if trigger_char.is_some()
            || (is_retrigger && (retrigger_char.is_some() || position != self.help_position))
        {
            let trigger_char = trigger_char.or(retrigger_char);

            doc.lsp_signature_help(trigger_char, false, ctx);

            self.help_position = position;

            self.help_path.clear();

            if let Some(path) = doc.path().some() {
                self.help_path.push(path);
            }
        }
    }

    pub fn draw(&self, tab: &Tab, doc: &Doc, ctx: &mut Ctx) -> Option<()> {
        let position = doc.cursor(CursorIndex::Main).position;

        let signature_help = self.help.as_ref()?;
        let active_signature = signature_help
            .signatures
            .get(signature_help.active_signature)?;

        let mut position = doc.position_to_visual(position, tab.camera.position(), ctx.gfx);
        position = position.offset_by(tab.doc_bounds());

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        if let Some(documentation) = &active_signature.documentation {
            let documentation_bounds = draw_popup(
                documentation.text(),
                position,
                PopupAlignment::Above,
                theme.normal,
                theme,
                gfx,
            );

            position.x = documentation_bounds.x + gfx.glyph_width();
            position.y -= documentation_bounds.height - gfx.border_width();
        }

        draw_popup(
            &active_signature.label,
            position,
            PopupAlignment::Above,
            theme.subtle,
            theme,
            gfx,
        );

        Some(())
    }

    pub fn get_triggers(
        &mut self,
        widget: &Widget,
        ui: &mut Ui,
        doc: Option<&mut Doc>,
        ctx: &mut Ctx,
    ) -> (Option<char>, Option<char>) {
        let mut trigger_char = None;
        let mut retrigger_char = None;

        let Some(language_server) =
            doc.and_then(|doc| ctx.lsp.get_language_server_mut(doc, ctx.config))
        else {
            return (trigger_char, retrigger_char);
        };

        let mut grapheme_handler = ui.grapheme_handler(widget, ctx.window);

        while let Some(c) = grapheme_handler
            .next(ctx.window)
            .and_then(|grapheme| grapheme.chars().nth(0))
        {
            if language_server.is_trigger_char(c) {
                trigger_char = Some(c);
            }

            if language_server.is_retrigger_char(c) {
                retrigger_char = Some(c);
            }

            grapheme_handler.unprocessed(ctx.window);
        }

        (trigger_char, retrigger_char)
    }

    pub fn lsp_set_signature_help(&mut self, help: Option<SignatureHelp>) {
        self.help = help;
    }

    pub fn is_open(&self) -> bool {
        self.help.is_some()
    }

    pub fn clear(&mut self) {
        self.help = None;
    }
}
