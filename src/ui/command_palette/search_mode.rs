use crate::{
    geometry::position::Position,
    platform::gfx::Gfx,
    text::{cursor_index::CursorIndex, doc::Doc, selection::Selection},
    ui::{editor::Editor, result_list::ResultListSubmitKind, tab::Tab},
};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction,
};

pub struct SearchMode {
    start: Position,
}

impl SearchMode {
    pub fn new() -> Self {
        Self {
            start: Position::ZERO,
        }
    }
}

impl CommandPaletteMode for SearchMode {
    fn title(&self) -> &str {
        "Search"
    }

    fn on_open(&mut self, _: &mut CommandPalette, args: CommandPaletteEventArgs) {
        self.start = get_start(args.editor);
    }

    fn on_update_results(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
    ) {
        preview_search(self.start, command_palette, args.editor, args.ctx.gfx);
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
        kind: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        let search_term = command_palette.get_input();

        let (pane, doc_list) = args.editor.get_focused_pane_and_doc_list_mut();
        let focused_tab_index = pane.focused_tab_index();

        let Some((tab, doc)) = pane.get_tab_with_data_mut(focused_tab_index, doc_list) else {
            return CommandPaletteAction::Stay;
        };

        search(
            search_term,
            None,
            tab,
            doc,
            kind == ResultListSubmitKind::Alternate,
            args.ctx.gfx,
        );

        CommandPaletteAction::Stay
    }
}

pub struct SearchAndReplaceMode {
    start: Position,
    search_term: Option<String>,
}

impl SearchAndReplaceMode {
    pub fn new() -> Self {
        Self {
            start: Position::ZERO,
            search_term: None,
        }
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

    fn on_open(&mut self, _: &mut CommandPalette, args: CommandPaletteEventArgs) {
        self.start = get_start(args.editor);
    }

    fn on_update_results(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
    ) {
        if self.search_term.is_some() {
            return;
        }

        preview_search(self.start, command_palette, args.editor, args.ctx.gfx);
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
        kind: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        let Some(search_term) = &self.search_term else {
            self.search_term = Some(command_palette.get_input().into());
            command_palette.doc.clear(args.ctx);

            return CommandPaletteAction::Stay;
        };

        let replace_term = command_palette.get_input();

        let (pane, doc_list) = args.editor.get_focused_pane_and_doc_list_mut();
        let focused_tab_index = pane.focused_tab_index();

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
                    doc.insert_at_cursor(CursorIndex::Main, replace_term, args.ctx);

                    let end =
                        doc.move_position(start, replace_term.len() as isize, 0, args.ctx.gfx);

                    doc.jump_cursor(CursorIndex::Main, start, false, args.ctx.gfx);
                    doc.jump_cursor(CursorIndex::Main, end, true, args.ctx.gfx);

                    return CommandPaletteAction::Stay;
                }
            }
        }

        search(
            search_term,
            None,
            tab,
            doc,
            kind == ResultListSubmitKind::Alternate,
            args.ctx.gfx,
        );

        CommandPaletteAction::Stay
    }
}

fn get_start(editor: &mut Editor) -> Position {
    let (pane, doc_list) = editor.get_focused_pane_and_doc_list();
    let focused_tab_index = pane.focused_tab_index();

    let Some((_, doc)) = pane.get_tab_with_data(focused_tab_index, doc_list) else {
        return Position::ZERO;
    };

    doc.get_cursor(CursorIndex::Main).position
}

fn preview_search(
    start: Position,
    command_palette: &CommandPalette,
    editor: &mut Editor,
    gfx: &mut Gfx,
) {
    let search_term = command_palette.get_input();

    let (pane, doc_list) = editor.get_focused_pane_and_doc_list_mut();
    let focused_tab_index = pane.focused_tab_index();

    let Some((tab, doc)) = pane.get_tab_with_data_mut(focused_tab_index, doc_list) else {
        return;
    };

    search(search_term, Some(start), tab, doc, false, gfx);
}

fn search(
    search_term: &str,
    start: Option<Position>,
    tab: &mut Tab,
    doc: &mut Doc,
    is_reverse: bool,
    gfx: &mut Gfx,
) {
    let start = start.unwrap_or(doc.get_cursor(CursorIndex::Main).position);

    if let Some(position) = doc.search(search_term, start, is_reverse, gfx) {
        let end = doc.move_position(position, search_term.len() as isize, 0, gfx);

        doc.jump_cursors(position, false, gfx);
        doc.jump_cursors(end, true, gfx);

        tab.camera.recenter();
    }
}
