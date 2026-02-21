use crate::{
    ctx::Ctx,
    geometry::position::Position,
    text::{cursor_index::CursorIndex, doc::Doc},
    ui::{
        core::Ui,
        slot_list::{SlotId, SlotList},
        widget_list::WidgetList,
    },
};

use super::editor_pane::EditorPane;

const CURSOR_POSITION_HISTORY_THRESHOLD: usize = 10;

struct CursorHistoryItem {
    position: Position,
    doc_id: SlotId,
}

impl CursorHistoryItem {
    fn new(position: Position, doc_id: SlotId) -> Self {
        Self { position, doc_id }
    }
}

pub struct CursorHistory {
    undo_history: Vec<CursorHistoryItem>,
    redo_history: Vec<CursorHistoryItem>,
    did_just_undo_redo: bool,
}

impl CursorHistory {
    pub fn new() -> Self {
        Self {
            undo_history: Vec::new(),
            redo_history: Vec::new(),
            did_just_undo_redo: false,
        }
    }

    pub fn update(
        &mut self,
        last_doc_id: Option<SlotId>,
        doc_id: SlotId,
        last_position: Option<Position>,
        position: Position,
    ) -> Option<()> {
        if self.did_just_undo_redo {
            self.did_just_undo_redo = false;
            return None;
        }

        let last_position = last_position?;
        let last_doc_id = last_doc_id?;

        if doc_id == last_doc_id
            && position.y.abs_diff(last_position.y) < CURSOR_POSITION_HISTORY_THRESHOLD
        {
            return None;
        }

        self.redo_history.clear();
        self.undo_history
            .push(CursorHistoryItem::new(last_position, last_doc_id));

        Some(())
    }

    pub fn undo(
        &mut self,
        panes: &mut WidgetList<EditorPane>,
        doc_list: &mut SlotList<Doc>,
        ctx: &mut Ctx,
    ) {
        self.did_just_undo_redo = true;

        Self::pop_item(
            &mut self.undo_history,
            &mut self.redo_history,
            panes,
            doc_list,
            ctx,
        );
    }

    pub fn redo(
        &mut self,
        panes: &mut WidgetList<EditorPane>,
        doc_list: &mut SlotList<Doc>,
        ctx: &mut Ctx,
    ) {
        self.did_just_undo_redo = true;

        Self::pop_item(
            &mut self.redo_history,
            &mut self.undo_history,
            panes,
            doc_list,
            ctx,
        );
    }

    fn pop_item(
        pop_history: &mut Vec<CursorHistoryItem>,
        push_history: &mut Vec<CursorHistoryItem>,
        panes: &mut WidgetList<EditorPane>,
        doc_list: &mut SlotList<Doc>,
        ctx: &mut Ctx,
    ) -> Option<()> {
        if pop_history.is_empty() {
            return None;
        }

        push_history.push(Self::get_item(panes, doc_list, ctx.ui)?);

        while let Some(item) = pop_history.pop() {
            if Self::jump_to_item(item, panes, doc_list, ctx) {
                break;
            }
        }

        Some(())
    }

    fn get_item(
        panes: &WidgetList<EditorPane>,
        doc_list: &SlotList<Doc>,
        ui: &Ui,
    ) -> Option<CursorHistoryItem> {
        let pane = panes.get_last_focused(ui)?;
        let doc_id = pane.get_focused_tab(ui)?.data_id();

        let doc = doc_list.get(doc_id)?;
        let cursor = doc.cursor(CursorIndex::Main);

        Some(CursorHistoryItem::new(cursor.position, doc_id))
    }

    fn jump_to_item(
        item: CursorHistoryItem,
        panes: &mut WidgetList<EditorPane>,
        doc_list: &mut SlotList<Doc>,
        ctx: &mut Ctx,
    ) -> bool {
        let Some(doc) = doc_list.get_mut(item.doc_id) else {
            return false;
        };

        let focused_pane = panes.get_last_focused_mut(ctx.ui).unwrap();

        if !Self::focus_tab_for_doc_id(focused_pane, item.doc_id, ctx.ui) {
            for pane in panes.iter_mut() {
                if !Self::focus_tab_for_doc_id(pane, item.doc_id, ctx.ui) {
                    continue;
                }

                ctx.ui.focus(pane.widget_id());
                break;
            }
        }

        doc.jump_cursor(CursorIndex::Main, item.position, false, ctx.gfx);
        true
    }

    fn focus_tab_for_doc_id(pane: &mut EditorPane, doc_id: SlotId, ui: &mut Ui) -> bool {
        let Some(index) = pane.iter_tabs().position(|tab| tab.data_id() == doc_id) else {
            return false;
        };

        pane.set_focused_tab_index(index, ui);
        true
    }
}
