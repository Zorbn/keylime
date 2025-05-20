use crate::{
    ctx::Ctx,
    geometry::position::Position,
    lsp::types::{DecodedDiagnostic, DecodedHover, DecodedRange},
    pool::Pooled,
    text::doc::Doc,
    ui::{
        core::{Ui, WidgetId},
        popup::{Popup, PopupAlignment},
        tab::Tab,
    },
};

#[derive(Debug)]
enum ExaminePopupData<'a> {
    None,
    Diagnostic(&'a DecodedDiagnostic),
    Hover(Pooled<String>, Option<DecodedRange>),
}

#[derive(Debug, PartialEq, Eq)]
enum ExaminePopupKind {
    None,
    Diagnostic,
    Hover,
}

pub struct ExaminePopup {
    popup: Popup,
    kind: ExaminePopupKind,
    position: Position,
    open_position: Position,
}

impl ExaminePopup {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        Self {
            popup: Popup::new(parent_id, ui),
            kind: ExaminePopupKind::None,
            position: Position::ZERO,
            open_position: Position::ZERO,
        }
    }

    pub fn layout(&self, tab: &Tab, doc: &Doc, ctx: &mut Ctx) {
        let mut position = doc.position_to_visual(self.position, tab.camera.position(), ctx.gfx);
        position = position.offset_by(tab.doc_bounds());

        let is_position_visible = tab.doc_bounds().contains_position(position);

        ctx.ui.set_shown(
            self.popup.widget_id(),
            self.kind != ExaminePopupKind::None && is_position_visible,
        );

        self.popup.layout(position, PopupAlignment::Above, ctx);
    }

    pub fn update(&mut self, did_cursor_move: bool, doc: &Doc, ctx: &mut Ctx) {
        let needs_clear = match self.kind {
            ExaminePopupKind::None => false,
            ExaminePopupKind::Diagnostic => {
                did_cursor_move || ctx.lsp.get_diagnostic_at(self.open_position, doc).is_none()
            }
            ExaminePopupKind::Hover => did_cursor_move,
        };

        if needs_clear {
            self.clear(ctx.ui);
        }
    }

    pub fn draw(&self, ctx: &mut Ctx) {
        let theme = &ctx.config.theme;

        self.popup.draw(theme.normal, ctx);
    }

    pub fn open(&mut self, position: Position, doc: &mut Doc, ctx: &mut Ctx) {
        self.open_position = position;

        if let Some(diagnostic) = ctx
            .lsp
            .get_diagnostic_at(position, doc)
            .filter(|_| self.kind != ExaminePopupKind::Diagnostic)
        {
            self.set_data(ExaminePopupData::Diagnostic(diagnostic), doc, ctx.ui);
        } else {
            doc.lsp_hover(position, ctx);
        }
    }

    pub fn lsp_set_hover(&mut self, hover: Option<DecodedHover>, doc: &Doc, ui: &mut Ui) {
        let data = match hover {
            Some(hover) => ExaminePopupData::Hover(hover.contents.text(), hover.range),
            None => ExaminePopupData::None,
        };

        self.set_data(data, doc, ui);
    }

    pub fn is_open(&self) -> bool {
        self.kind != ExaminePopupKind::None
    }

    pub fn clear(&mut self, ui: &mut Ui) {
        self.kind = ExaminePopupKind::None;
        self.popup.hide(ui);
    }

    fn set_data(&mut self, kind: ExaminePopupData, doc: &Doc, ui: &mut Ui) {
        self.clear(ui);

        match kind {
            ExaminePopupData::None => {}
            ExaminePopupData::Diagnostic(diagnostic) => {
                self.popup.show(&diagnostic.message, ui);
                self.position = diagnostic.visible_range(doc).start;
                self.kind = ExaminePopupKind::Diagnostic;
            }
            ExaminePopupData::Hover(text, range) => {
                self.popup.show(&text, ui);
                self.position = range.map(|range| range.start).unwrap_or(self.open_position);
                self.kind = ExaminePopupKind::Hover;
            }
        }
    }

    pub fn widget_id(&self) -> WidgetId {
        self.popup.widget_id()
    }
}
