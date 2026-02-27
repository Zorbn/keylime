use crate::{
    ctx::Ctx,
    geometry::{position::Position, rect::Rect},
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
    pub fn new(parent_id: WidgetId, ctx: &mut Ctx) -> Self {
        let widget_id = ctx.ui.new_widget(
            parent_id,
            WidgetSettings {
                popup: Some(Rect::ZERO),
                wants_msgs: false,
                is_owned_by_parent: false,
                ..Default::default()
            },
        );

        Self {
            widget_id,

            help_position: Position::ZERO,
            help: None,

            label_popup: Popup::new(widget_id, ctx),
            documentation_popup: Popup::new(widget_id, ctx),
        }
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.label_popup.is_animating(ctx) || self.documentation_popup.is_animating(ctx)
    }

    pub fn receive_msgs(&mut self, ctx: &mut Ctx) {
        self.label_popup.receive_msgs(ctx);
        self.documentation_popup.receive_msgs(ctx);
    }

    pub fn show(
        &mut self,
        doc: &mut Doc,
        parent_id: WidgetId,
        trigger_char: char,
        is_retrigger: bool,
        ctx: &mut Ctx,
    ) {
        if is_retrigger && self.help.is_none() {
            return;
        }

        let position = doc.cursor(CursorIndex::Main).position;

        doc.lsp_signature_help(Some(trigger_char), is_retrigger, ctx);

        ctx.ui.reparent_widget(self.widget_id, parent_id);

        self.help_position = position;
    }

    pub fn update(&mut self, tab: &Tab, doc: &mut Doc, ctx: &mut Ctx) {
        if self.help.is_some() && doc.cursor(CursorIndex::Main).position != self.help_position {
            doc.lsp_signature_help(None, true, ctx);
        }

        let mut position = doc
            .position_to_visual(self.help_position, tab.camera.position(), ctx.gfx)
            .offset_by(tab.doc_bounds(ctx.ui));

        if ctx.ui.is_visible(self.documentation_popup.widget_id()) {
            self.documentation_popup
                .update(position, PopupAlignment::Above, ctx);

            let documentation_bounds = ctx.ui.bounds(self.documentation_popup.widget_id());

            position.x = documentation_bounds.x + ctx.gfx.glyph_width();
            position.y -= documentation_bounds.height - ctx.gfx.border_width();
        }

        self.label_popup
            .update(position, PopupAlignment::Above, ctx);
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

        self.label_popup.show(&active_signature.label, "", ctx);

        let documentation = active_signature.documentation.as_ref()?;

        self.documentation_popup
            .show(documentation.text(), documentation.extension(), ctx);

        Some(())
    }

    pub fn is_open(&self) -> bool {
        self.help.is_some()
    }

    pub fn hide(&mut self, ui: &mut Ui) {
        self.help = None;

        self.label_popup.hide(ui);
        self.documentation_popup.hide(ui);
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }
}
