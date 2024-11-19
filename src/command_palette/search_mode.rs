use crate::{cursor_index::CursorIndex, editor::Editor, line_pool::LinePool, position::Position};

use super::{mode::CommandPaletteMode, CommandPalette};

pub const MODE_SEARCH: CommandPaletteMode = CommandPaletteMode {
    title: "Search",
    on_submit: on_submit_search,
    on_complete_result: |_, _, _| {},
    on_update_results: |_, _, _| {},
    on_backspace: |_, _, _| false,
};

fn on_submit_search(
    command_palette: &mut CommandPalette,
    has_shift: bool,
    editor: &mut Editor,
    _: &mut LinePool,
    _: f32,
) -> bool {
    let focused_tab_index = editor.focused_tab_index();

    let Some(search_term) = command_palette.doc.get_line(0) else {
        return false;
    };

    let Some((tab, doc)) = editor.get_tab_with_doc(focused_tab_index) else {
        return false;
    };

    let start = doc.get_cursor(CursorIndex::Main).position;

    let result = if has_shift {
        doc.search_backwards(search_term, start, true)
    } else {
        doc.search_forwards(search_term, start, true)
    };

    if let Some(position) = result {
        let end = doc.move_position(position, Position::new(search_term.len() as isize, 0));

        doc.jump_cursors(position, false);
        doc.jump_cursors(end, true);

        tab.recenter();
    }

    false
}
