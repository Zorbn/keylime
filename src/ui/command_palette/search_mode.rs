use crate::{
    platform::gfx::Gfx,
    text::{cursor_index::CursorIndex, doc::Doc, selection::Selection},
    ui::{result_list::ResultListSubmitKind, tab::Tab},
};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction,
};

pub const MODE_SEARCH: &CommandPaletteMode = &CommandPaletteMode {
    title: "Search",
    on_submit: on_submit_search,
    ..CommandPaletteMode::default()
};

pub const MODE_SEARCH_AND_REPLACE_START: &CommandPaletteMode = &CommandPaletteMode {
    title: "Search and Replace: Search",
    on_submit: on_submit_search_and_replace_start,
    do_passthrough_result: true,
    ..CommandPaletteMode::default()
};

pub const MODE_SEARCH_AND_REPLACE_END: &CommandPaletteMode = &CommandPaletteMode {
    title: "Search and Replace: Replace",
    on_submit: on_submit_search_and_replace_end,
    ..CommandPaletteMode::default()
};

fn on_submit_search(
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

    let Some(search_term) = command_palette.doc.get_line(0) else {
        return CommandPaletteAction::Stay;
    };

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

fn on_submit_search_and_replace_start(
    _: &mut CommandPalette,
    _: CommandPaletteEventArgs,
    kind: ResultListSubmitKind,
) -> CommandPaletteAction {
    if kind != ResultListSubmitKind::Normal {
        return CommandPaletteAction::Stay;
    }

    CommandPaletteAction::Open(MODE_SEARCH_AND_REPLACE_END)
}

fn on_submit_search_and_replace_end(
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

    let Some(search_term) = command_palette.previous_results.last() else {
        return CommandPaletteAction::Stay;
    };

    let Some(replace_term) = command_palette.doc.get_line(0) else {
        return CommandPaletteAction::Stay;
    };

    let Some((tab, doc)) = pane.get_tab_with_data_mut(focused_tab_index, doc_list) else {
        return CommandPaletteAction::Stay;
    };

    if kind == ResultListSubmitKind::Normal && doc.cursors_len() == 1 {
        if let Some(Selection { start, end }) = doc.get_cursor(CursorIndex::Main).get_selection() {
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

fn search(search_term: &str, tab: &mut Tab, doc: &mut Doc, is_reverse: bool, gfx: &mut Gfx) {
    let start = doc.get_cursor(CursorIndex::Main).position;

    if let Some(position) = doc.search(search_term, start, is_reverse, gfx) {
        let end = doc.move_position(position, search_term.len() as isize, 0, gfx);

        doc.jump_cursors(position, false, gfx);
        doc.jump_cursors(end, true, gfx);

        tab.camera.recenter();
    }
}
