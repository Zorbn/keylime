use crate::{
    ctx::Ctx,
    geometry::position::Position,
    lsp::types::{DecodedDiagnostic, Hover},
    pool::Pooled,
    text::{cursor_index::CursorIndex, doc::Doc},
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
    Hover(Pooled<String>),
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
}

impl ExaminePopup {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        Self {
            // TODO: Do visibility updates for this.
            popup: Popup::new(parent_id, ui),
            kind: ExaminePopupKind::None,
            position: Position::ZERO,
        }
    }

    pub fn layout(&mut self, tab: &Tab, doc: &Doc, ui: &mut Ui, ctx: &mut Ctx) {
        if self.kind == ExaminePopupKind::None {
            ui.hide(self.popup.widget_id());
            return;
        } else {
            ui.show(self.popup.widget_id());
        }

        let mut position = doc.position_to_visual(self.position, tab.camera.position(), ctx.gfx);
        position = position.offset_by(tab.doc_bounds());

        self.popup
            .layout(position, PopupAlignment::Above, ui, ctx.gfx);
    }

    pub fn update(&mut self, did_cursor_move: bool, doc: &Doc, ctx: &mut Ctx) {
        let position = doc.cursor(CursorIndex::Main).position;

        let needs_clear = match self.kind {
            ExaminePopupKind::None => false,
            ExaminePopupKind::Diagnostic => ctx.lsp.get_diagnostic_at(position, doc).is_none(),
            ExaminePopupKind::Hover => did_cursor_move,
        };

        if needs_clear {
            self.clear();
        }
    }

    pub fn draw(&self, ui: &Ui, ctx: &mut Ctx) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        self.popup.draw(theme.normal, theme, ui, gfx);
    }

    pub fn open(&mut self, doc: &mut Doc, ctx: &mut Ctx) {
        let position = doc.cursor(CursorIndex::Main).position;

        if let Some(diagnostic) = ctx
            .lsp
            .get_diagnostic_at(position, doc)
            .filter(|_| self.kind != ExaminePopupKind::Diagnostic)
        {
            self.set_data(ExaminePopupData::Diagnostic(diagnostic), doc);
        } else {
            doc.lsp_hover(ctx);
        }
    }

    pub fn lsp_set_hover(&mut self, hover: Option<Hover>, doc: &Doc) {
        let data = match hover {
            Some(hover) => ExaminePopupData::Hover(hover.contents.text()),
            None => ExaminePopupData::None,
        };

        self.set_data(data, doc);
    }

    pub fn is_open(&self) -> bool {
        self.kind != ExaminePopupKind::None
    }

    pub fn clear(&mut self) {
        self.kind = ExaminePopupKind::None;
        self.popup.text.clear();
    }

    fn set_data(&mut self, kind: ExaminePopupData, doc: &Doc) {
        self.clear();

        match kind {
            ExaminePopupData::None => {}
            ExaminePopupData::Diagnostic(diagnostic) => {
                self.popup.text.push_str(&diagnostic.message);
                self.position = diagnostic.visible_range(doc).start;
                self.kind = ExaminePopupKind::Diagnostic;
            }
            ExaminePopupData::Hover(text) => {
                self.popup.text.push_str(&text);
                self.position = doc.cursor(CursorIndex::Main).position;
                self.kind = ExaminePopupKind::Hover;
            }
        }
    }
}
