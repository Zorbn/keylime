use crate::{
    ctx::Ctx,
    geometry::position::Position,
    lsp::types::SignatureHelp,
    text::{cursor_index::CursorIndex, doc::Doc},
    ui::{
        core::{Ui, WidgetId, WidgetSettings},
        popup::{Popup, PopupAlignment},
        tab::Tab,
    },
};

pub struct SignatureHelpPopup {
    widget_id: WidgetId,

    help_position: Position,
    help: Option<SignatureHelp>,

    label_popup: Popup,
    documentation_popup: Popup,
}

impl SignatureHelpPopup {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        let widget_id = ui.new_widget(parent_id, WidgetSettings::default());

        Self {
            widget_id,

            help_position: Position::ZERO,
            help: None,

            label_popup: Popup::new(widget_id, ui),
            documentation_popup: Popup::new(widget_id, ui),
        }
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.label_popup.is_animating(ctx) || self.documentation_popup.is_animating(ctx)
    }

    pub fn layout(&mut self, tab: &Tab, doc: &Doc, ctx: &mut Ctx) -> Option<()> {
        let position = doc.cursor(CursorIndex::Main).position;

        let mut position = doc.position_to_visual(position, tab.camera.position(), ctx.gfx);
        position = position.offset_by(tab.doc_bounds());

        if ctx.ui.is_visible(self.documentation_popup.widget_id()) {
            self.documentation_popup
                .layout(position, PopupAlignment::Above, ctx);

            let documentation_bounds = ctx.ui.widget(self.documentation_popup.widget_id()).bounds;

            position.x = documentation_bounds.x + ctx.gfx.glyph_width();
            position.y -= documentation_bounds.height - ctx.gfx.border_width();
        }

        self.label_popup
            .layout(position, PopupAlignment::Above, ctx);

        Some(())
    }

    pub fn update(
        &mut self,
        is_doc_different: bool,
        (trigger_char, retrigger_char): (Option<char>, Option<char>),
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) {
        if is_doc_different {
            self.clear(ctx.ui);
        }

        let position = doc.cursor(CursorIndex::Main).position;
        let is_retrigger = self.help.is_some();

        if trigger_char.is_some()
            || (is_retrigger && (retrigger_char.is_some() || position != self.help_position))
        {
            let trigger_char = trigger_char.or(retrigger_char);

            doc.lsp_signature_help(trigger_char, false, ctx);

            self.help_position = position;
        }

        self.label_popup.update(ctx);
        self.documentation_popup.update(ctx);
    }

    pub fn animate(&mut self, ctx: &mut Ctx, dt: f32) {
        self.label_popup.animate(ctx, dt);
        self.documentation_popup.animate(ctx, dt);
    }

    pub fn draw(&mut self, ctx: &mut Ctx) -> Option<()> {
        let theme = &ctx.config.theme;

        self.label_popup.draw(Some(theme.subtle), ctx);
        self.documentation_popup.draw(None, ctx);

        Some(())
    }

    pub fn get_triggers(
        widget_id: WidgetId,
        doc: Option<&mut Doc>,
        ctx: &mut Ctx,
    ) -> (Option<char>, Option<char>) {
        let mut trigger_char = None;
        let mut retrigger_char = None;

        let Some(language_server) = doc.and_then(|doc| {
            ctx.lsp
                .get_language_server_mut(doc, ctx.config, ctx.current_dir)
        }) else {
            return (trigger_char, retrigger_char);
        };

        let mut grapheme_handler = ctx.ui.grapheme_handler(widget_id, ctx.window);

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
        ctx: &mut Ctx,
    ) -> Option<()> {
        self.help = help;

        self.label_popup.hide(ctx.ui);
        self.documentation_popup.hide(ctx.ui);

        let signature_help = self.help.as_ref()?;
        let active_signature = signature_help
            .signatures
            .get(signature_help.active_signature)?;

        self.label_popup.show(&active_signature.label, ctx);

        let documentation = active_signature.documentation.as_ref()?;
        self.documentation_popup.show(documentation.text(), ctx);

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

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }
}
