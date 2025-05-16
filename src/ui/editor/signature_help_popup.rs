use std::path::PathBuf;

use crate::{
    ctx::Ctx,
    geometry::position::Position,
    lsp::types::SignatureHelp,
    platform::gfx::Gfx,
    text::{cursor_index::CursorIndex, doc::Doc},
    ui::{
        core::{Ui, WidgetId},
        popup::{Popup, PopupAlignment},
        tab::Tab,
    },
};

pub struct SignatureHelpPopup {
    help_path: PathBuf,
    help_position: Position,
    help: Option<SignatureHelp>,

    label_popup: Popup,
    documentation_popup: Popup,
}

impl SignatureHelpPopup {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        Self {
            help_path: PathBuf::new(),
            help_position: Position::ZERO,
            help: None,

            // TODO: Do visiblity updates for this if necessary.
            label_popup: Popup::new(parent_id, ui),
            documentation_popup: Popup::new(parent_id, ui),
        }
    }

    pub fn layout(&mut self, tab: &Tab, doc: &Doc, ui: &mut Ui, gfx: &mut Gfx) -> Option<()> {
        let position = doc.cursor(CursorIndex::Main).position;

        let mut position = doc.position_to_visual(position, tab.camera.position(), gfx);
        position = position.offset_by(tab.doc_bounds());

        if ui.is_visible(self.documentation_popup.widget_id()) {
            self.documentation_popup
                .layout(position, PopupAlignment::Above, ui, gfx);

            let documentation_bounds = ui.widget(self.documentation_popup.widget_id()).bounds;

            position.x = documentation_bounds.x + gfx.glyph_width();
            position.y -= documentation_bounds.height - gfx.border_width();
        }

        self.label_popup
            .layout(position, PopupAlignment::Above, ui, gfx);

        Some(())
    }

    pub fn update(
        &mut self,
        (trigger_char, retrigger_char): (Option<char>, Option<char>),
        doc: &mut Doc,
        ui: &mut Ui,
        ctx: &mut Ctx,
    ) {
        if Some(self.help_path.as_path()) != doc.path().some_path() {
            self.clear(ui);
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

    pub fn draw(&self, ui: &Ui, ctx: &mut Ctx) -> Option<()> {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        self.documentation_popup.draw(theme.normal, theme, ui, gfx);
        self.label_popup.draw(theme.subtle, theme, ui, gfx);

        Some(())
    }

    pub fn get_triggers(
        widget_id: WidgetId,
        ui: &Ui,
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

        let mut grapheme_handler = ui.grapheme_handler(widget_id, ctx.window);

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

    pub fn lsp_set_signature_help(
        &mut self,
        help: Option<SignatureHelp>,
        ui: &mut Ui,
    ) -> Option<()> {
        self.help = help;

        self.label_popup.hide(ui);
        self.documentation_popup.hide(ui);

        let signature_help = self.help.as_ref()?;
        let active_signature = signature_help
            .signatures
            .get(signature_help.active_signature)?;

        self.label_popup.show(&active_signature.label, ui);

        let documentation = active_signature.documentation.as_ref()?;
        self.documentation_popup.show(documentation.text(), ui);

        Some(())
    }

    pub fn is_open(&self) -> bool {
        self.help.is_some()
    }

    pub fn clear(&mut self, ui: &mut Ui) {
        self.help = None;

        self.label_popup.hide(ui);
        self.documentation_popup.hide(ui);
    }
}
