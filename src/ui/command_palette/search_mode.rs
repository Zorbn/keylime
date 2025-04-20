use crate::{
    platform::gfx::Gfx,
    text::{cursor_index::CursorIndex, doc::Doc, selection::Selection},
    ui::{result_list::ResultListSubmitKind, tab::Tab},
};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction,
};

pub struct SearchMode;

impl CommandPaletteMode for SearchMode {
    fn title(&self) -> &str {
        "Search"
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        CommandPaletteEventArgs {
            pane,
            doc_list,
            ctx,
            ..
        }: CommandPaletteEventArgs,
        kind: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        if !matches!(
            kind,
            ResultListSubmitKind::Normal | ResultListSubmitKind::Alternate
        ) {
            return CommandPaletteAction::Stay;
        }

        let focused_tab_index = pane.focused_tab_index();

        let search_term = command_palette.get_input();

        let Some((tab, doc)) = pane.get_tab_with_data_mut(focused_tab_index, doc_list) else {
            return CommandPaletteAction::Stay;
        };

        search(
            search_term,
            tab,
            doc,
            kind == ResultListSubmitKind::Alternate,
            ctx.gfx,
        );

        CommandPaletteAction::Stay
    }
}

pub struct SearchAndReplaceMode {
    search_term: Option<String>,
}

impl SearchAndReplaceMode {
    pub fn new() -> Self {
        Self { search_term: None }
    }
}

impl CommandPaletteMode for SearchAndReplaceMode {
    fn title(&self) -> &str {
        if self.search_term.is_none() {
            "Search and Replace: Search"
        } else {
            "Search and Replace: Replace"
        }
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        CommandPaletteEventArgs {
            pane,
            doc_list,
            ctx,
            ..
        }: CommandPaletteEventArgs,
        kind: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        if !matches!(
            kind,
            ResultListSubmitKind::Normal | ResultListSubmitKind::Alternate
        ) {
            return CommandPaletteAction::Stay;
        }

        let Some(search_term) = &self.search_term else {
            self.search_term = Some(command_palette.get_input().into());
            command_palette.doc.clear(&mut ctx.buffers.lines);

            return CommandPaletteAction::Stay;
        };

        let focused_tab_index = pane.focused_tab_index();

        let replace_term = command_palette.get_input();

        let Some((tab, doc)) = pane.get_tab_with_data_mut(focused_tab_index, doc_list) else {
            return CommandPaletteAction::Stay;
        };

        if kind == ResultListSubmitKind::Normal && doc.cursors_len() == 1 {
            if let Some(Selection { start, end }) =
                doc.get_cursor(CursorIndex::Main).get_selection()
            {
                let mut has_match = false;

                if start.y == end.y && end.x - start.x == search_term.len() {
                    if let Some(line) = doc.get_line(start.y) {
                        has_match = line[start.x..end.x] == *search_term;
                    }
                }

                if has_match {
                    doc.insert_at_cursor(CursorIndex::Main, replace_term, ctx);

                    let end = doc.move_position(start, replace_term.len() as isize, 0, ctx.gfx);

                    doc.jump_cursor(CursorIndex::Main, start, false, ctx.gfx);
                    doc.jump_cursor(CursorIndex::Main, end, true, ctx.gfx);

                    return CommandPaletteAction::Stay;
                }
            }
        }

        search(
            search_term,
            tab,
            doc,
            kind == ResultListSubmitKind::Alternate,
            ctx.gfx,
        );

        CommandPaletteAction::Stay
    }
}

fn search(search_term: &str, tab: &mut Tab, doc: &mut Doc, is_reverse: bool, gfx: &mut Gfx) {
    let start = doc.get_cursor(CursorIndex::Main).position;

    if let Some(position) = doc.search(search_term, start, is_reverse, gfx) {
        let end = doc.move_position(position, search_term.len() as isize, 0, gfx);

        doc.jump_cursors(position, false, gfx);
        doc.jump_cursors(end, true, gfx);

        tab.camera.recenter();
    }
}
