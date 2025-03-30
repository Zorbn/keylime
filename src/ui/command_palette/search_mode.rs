use crate::{
    geometry::position::Position,
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
    CommandPaletteEventArgs { pane, doc_list, .. }: CommandPaletteEventArgs,
    kind: ResultListSubmitKind,
) -> CommandPaletteAction {
    // if !matches!(
    //     kind,
    //     ResultListSubmitKind::Normal | ResultListSubmitKind::Alternate
    // ) {
    //     return CommandPaletteAction::Stay;
    // }

    // let focused_tab_index = pane.focused_tab_index();

    // let Some(search_term) = command_palette.doc.get_line(0) else {
    //     return CommandPaletteAction::Stay;
    // };

    // let Some((tab, doc)) = pane.get_tab_with_data_mut(focused_tab_index, doc_list) else {
    //     return CommandPaletteAction::Stay;
    // };

    // search(
    //     search_term,
    //     tab,
    //     doc,
    //     kind == ResultListSubmitKind::Alternate,
    // );

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
        line_pool,
        time,
        ..
    }: CommandPaletteEventArgs,
    kind: ResultListSubmitKind,
) -> CommandPaletteAction {
    // if !matches!(
    //     kind,
    //     ResultListSubmitKind::Normal | ResultListSubmitKind::Alternate
    // ) {
    //     return CommandPaletteAction::Stay;
    // }

    // let focused_tab_index = pane.focused_tab_index();

    // let Some(search_term) = command_palette.previous_results.last() else {
    //     return CommandPaletteAction::Stay;
    // };

    // let Some(replace_term) = command_palette.doc.get_line(0) else {
    //     return CommandPaletteAction::Stay;
    // };

    // let Some((tab, doc)) = pane.get_tab_with_data_mut(focused_tab_index, doc_list) else {
    //     return CommandPaletteAction::Stay;
    // };

    // if kind == ResultListSubmitKind::Normal && doc.cursors_len() == 1 {
    //     if let Some(Selection { start, end }) = doc.get_cursor(CursorIndex::Main).get_selection() {
    //         let mut has_match = false;
    //         let mut position = start;
    //         let mut i = 0;

    //         if start.y == end.y && end.x - start.x == search_term.len() as isize {
    //             has_match = true;

    //             while position < end {
    //                 if doc.get_grapheme(position) != search_term[i] {
    //                     has_match = false;
    //                     break;
    //                 }

    //                 position = doc.move_position(position, Position::new(1, 0));
    //                 i += 1;
    //             }
    //         }

    //         if has_match {
    //             doc.insert_at_cursor(CursorIndex::Main, replace_term, line_pool, time);

    //             let end = doc.move_position(start, Position::new(replace_term.len() as isize, 0));

    //             doc.jump_cursor(CursorIndex::Main, start, false);
    //             doc.jump_cursor(CursorIndex::Main, end, true);

    //             return CommandPaletteAction::Stay;
    //         }
    //     }
    // }

    // search(
    //     search_term,
    //     tab,
    //     doc,
    //     kind == ResultListSubmitKind::Alternate,
    // );

    CommandPaletteAction::Stay
}

fn search(search_term: &[char], tab: &mut Tab, doc: &mut Doc, is_reverse: bool) {
    // let start = doc.get_cursor(CursorIndex::Main).position;

    // if let Some(position) = doc.search(search_term, start, is_reverse) {
    //     let end = doc.move_position(position, Position::new(search_term.len() as isize, 0));

    //     doc.jump_cursors(position, false);
    //     doc.jump_cursors(end, true);

    //     tab.camera.recenter();
    // }
}
