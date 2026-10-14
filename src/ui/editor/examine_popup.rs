use crate::{
    ctx::Ctx,
    geometry::{position::Position, rect::Rect},
    lsp::types::{DecodedDiagnostic, DecodedHover, DecodedRange, HoverContents},
    text::doc::Doc,
    ui::{
        core::{Ui, WidgetId, WidgetSettings},
        editor::editor_pane::EditorPane,
        pane_list::PaneList,
        popup::{Popup, PopupAlignment},
        slot_list::SlotList,
    },
};

#[derive(Debug)]
enum ExaminePopupData<'a> {
    None,
    Diagnostic(&'a DecodedDiagnostic),
    Hover(HoverContents, Option<DecodedRange>),
}

#[derive(Debug, PartialEq, Eq)]
enum ExaminePopupKind {
    None,
    Diagnostic,
    Hover,
}

struct RequestedLocation {
    parent_id: WidgetId,
    position: Position,
}

pub struct ExaminePopup {
    widget_id: WidgetId,
    popup: Popup,
    kind: ExaminePopupKind,
    position: Position,
    requested_location: Option<RequestedLocation>,
}

impl ExaminePopup {
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
            popup: Popup::new(widget_id, ctx),
            kind: ExaminePopupKind::None,
            position: Position::ZERO,
            requested_location: None,
        }
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.popup.is_animating(ctx)
    }

    pub fn receive_msgs(&mut self, ctx: &mut Ctx) {
        self.popup.receive_msgs(ctx);
    }

    pub fn update(
        &mut self,
        panes: &PaneList<EditorPane, Doc>,
        doc_list: &SlotList<Doc>,
        ctx: &mut Ctx,
        dt: f32,
    ) {
        let Some((tab, doc)) = ctx
            .ui
            .parent_id(self.widget_id)
            .and_then(|parent_id| panes.get_tab_with_data_for_widget(parent_id, doc_list))
        else {
            return;
        };

        if matches!(self.kind, ExaminePopupKind::Diagnostic)
            && ctx.lsp.get_diagnostic_at(self.position, doc).is_none()
        {
            self.hide(ctx.ui);
        }

        let tab_bounds = tab.doc_bounds(ctx.ui);
        let position = doc
            .position_to_visual(self.position, tab.camera.position(), ctx.gfx)
            .offset_by(tab_bounds);

        let is_position_visible = tab_bounds.contains_position(position);

        ctx.ui.set_shown(
            self.widget_id,
            self.kind != ExaminePopupKind::None && is_position_visible,
        );

        self.popup.update(position, PopupAlignment::Above, ctx, dt);
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        self.popup.draw(None, ctx);
    }

    pub fn show(
        &mut self,
        position: Position,
        parent_id: WidgetId,
        do_toggle: bool,
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) {
        self.requested_location = Some(RequestedLocation {
            parent_id,
            position,
        });

        let can_open_diagnostic = !do_toggle || self.kind != ExaminePopupKind::Diagnostic;

        if let Some(diagnostic) = can_open_diagnostic
            .then(|| ctx.lsp.get_diagnostic_at(position, doc))
            .flatten()
        {
            let diagnostic = diagnostic.clone();

            self.set_data(ExaminePopupData::Diagnostic(&diagnostic), doc, ctx);
        } else {
            doc.lsp_hover(position, ctx);
        }
    }

    pub fn lsp_set_hover(&mut self, hover: Option<DecodedHover>, doc: &Doc, ctx: &mut Ctx) {
        let data = match hover {
            Some(hover) => ExaminePopupData::Hover(hover.contents, hover.range),
            None => ExaminePopupData::None,
        };

        self.set_data(data, doc, ctx);
    }

    pub fn is_open(&self) -> bool {
        self.kind != ExaminePopupKind::None
    }

    pub fn hide(&mut self, ui: &mut Ui) {
        self.kind = ExaminePopupKind::None;
        self.popup.hide(ui);
    }

    fn set_data(&mut self, kind: ExaminePopupData, doc: &Doc, ctx: &mut Ctx) {
        self.hide(ctx.ui);

        let Some(RequestedLocation {
            parent_id,
            position,
        }) = self.requested_location.take()
        else {
            return;
        };

        if !ctx.ui.exists(parent_id) {
            return;
        }

        ctx.ui.reparent_widget(self.widget_id, parent_id);

        match kind {
            ExaminePopupData::None => {}
            ExaminePopupData::Diagnostic(diagnostic) => {
                self.popup.show(&diagnostic.message, "", ctx);
                self.position = diagnostic.visible_range(doc).start;
                self.kind = ExaminePopupKind::Diagnostic;
            }
            ExaminePopupData::Hover(contents, range) => {
                self.popup.show(&contents.text(), contents.extension(), ctx);
                self.position = range.map(|range| range.start).unwrap_or(position);
                self.kind = ExaminePopupKind::Hover;
            }
        }
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }
}
