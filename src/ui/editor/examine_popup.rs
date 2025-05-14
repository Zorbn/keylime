use crate::{
    ctx::Ctx,
    geometry::position::Position,
    lsp::types::Hover,
    pool::Pooled,
    text::{cursor_index::CursorIndex, doc::Doc},
    ui::{
        popup::{draw_popup, PopupAlignment},
        tab::Tab,
    },
};

#[derive(Debug, PartialEq, Eq)]
enum ExaminePopupKind {
    None,
    Diagnostic,
    Hover(Pooled<String>),
}

pub struct ExaminePopup {
    kind: ExaminePopupKind,
}

impl ExaminePopup {
    pub fn new() -> Self {
        Self {
            kind: ExaminePopupKind::None,
        }
    }

    pub fn update(&mut self, handled_position: Option<Position>, doc: &Doc, ctx: &mut Ctx) {
        let position = doc.cursor(CursorIndex::Main).position;

        let needs_clear = match self.kind {
            ExaminePopupKind::None => false,
            ExaminePopupKind::Diagnostic => ctx.lsp.get_diagnostic_at(position, doc).is_none(),
            ExaminePopupKind::Hover(..) => Some(position) != handled_position,
        };

        if needs_clear {
            self.clear();
        }
    }

    pub fn draw(&self, tab: &Tab, doc: &Doc, ctx: &mut Ctx) {
        match &self.kind {
            ExaminePopupKind::None => None,
            ExaminePopupKind::Diagnostic => self.draw_diagnostic_popup(tab, doc, ctx),
            ExaminePopupKind::Hover(text) => Self::draw_hover_popup(text, tab, doc, ctx),
        };
    }

    fn draw_diagnostic_popup(&self, tab: &Tab, doc: &Doc, ctx: &mut Ctx) -> Option<()> {
        let position = doc.cursor(CursorIndex::Main).position;
        let diagnostic = ctx.lsp.get_diagnostic_at(position, doc)?;

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        let start = diagnostic.visible_range(doc).start;
        let mut position = doc.position_to_visual(start, tab.camera.position(), gfx);
        position = position.offset_by(tab.doc_bounds());

        draw_popup(
            &diagnostic.message,
            position,
            PopupAlignment::Above,
            theme.normal,
            theme,
            gfx,
        );

        Some(())
    }

    fn draw_hover_popup(text: &str, tab: &Tab, doc: &Doc, ctx: &mut Ctx) -> Option<()> {
        let position = doc.cursor(CursorIndex::Main).position;

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        let mut position = doc.position_to_visual(position, tab.camera.position(), gfx);
        position = position.offset_by(tab.doc_bounds());

        draw_popup(
            text,
            position,
            PopupAlignment::Above,
            theme.normal,
            theme,
            gfx,
        );

        Some(())
    }

    pub fn open(&mut self, doc: &mut Doc, ctx: &mut Ctx) {
        let position = doc.cursor(CursorIndex::Main).position;

        let needs_diagnostic_popup = self.kind != ExaminePopupKind::Diagnostic
            && ctx.lsp.get_diagnostic_at(position, doc).is_some();

        if needs_diagnostic_popup {
            self.kind = ExaminePopupKind::Diagnostic;
        } else {
            doc.lsp_hover(ctx);
        }
    }

    pub fn lsp_set_hover(&mut self, hover: Option<Hover>) {
        self.kind = match hover {
            Some(hover) => ExaminePopupKind::Hover(hover.contents.text()),
            None => ExaminePopupKind::None,
        };
    }

    pub fn is_open(&self) -> bool {
        self.kind != ExaminePopupKind::None
    }

    pub fn clear(&mut self) {
        self.kind = ExaminePopupKind::None;
    }
}
