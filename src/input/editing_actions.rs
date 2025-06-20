use std::cmp::Ordering;

use crate::{
    ctx::Ctx,
    geometry::position::Position,
    platform::gfx::Gfx,
    pool::STRING_POOL,
    text::{
        action_history::ActionKind,
        cursor::Cursor,
        cursor_index::CursorIndex,
        doc::{Doc, DocFlag},
        grapheme,
        selection::Selection,
    },
    ui::tab::Tab,
};

use super::{
    action::{action_keybind, action_name, Action},
    mods::{Mod, Mods},
    mousebind::MouseClickCount,
};

pub enum DeleteKind {
    Char,
    Word,
    Line,
}

pub fn handle_grapheme(grapheme: &str, doc: &mut Doc, ctx: &mut Ctx) {
    if !doc.flags().contains(DocFlag::MatchPairs) {
        for index in doc.cursor_indices() {
            doc.insert_at_cursor(index, grapheme, ctx);
        }

        return;
    }

    for index in doc.cursor_indices() {
        let cursor = doc.cursor(index);

        let next_grapheme = doc.grapheme(cursor.position);

        let previous_position = doc.move_position(cursor.position, -1, 0, ctx.gfx);
        let previous_grapheme = doc.grapheme(previous_position);

        let matching_grapheme = get_matching_grapheme(grapheme);

        if let Some((matching_grapheme, selection)) = matching_grapheme.zip(cursor.get_selection())
        {
            doc.insert(selection.end, matching_grapheme, ctx);
            doc.set_cursor_selection(index, Some(selection));
            doc.insert(selection.start, grapheme, ctx);

            continue;
        }

        if is_matching_grapheme(grapheme) && next_grapheme == grapheme {
            doc.move_cursor(index, 1, 0, false, ctx.gfx);

            continue;
        }

        if let Some(matching_grapheme) = matching_grapheme
            .filter(|_| should_insert_matching_grapheme(grapheme, next_grapheme, previous_grapheme))
        {
            doc.insert_at_cursor(index, grapheme, ctx);
            doc.insert_at_cursor(index, matching_grapheme, ctx);
            doc.move_cursor(index, -1, 0, false, ctx.gfx);

            continue;
        }

        doc.insert_at_cursor(index, grapheme, ctx);
    }
}

fn should_insert_matching_grapheme(
    grapheme: &str,
    next_grapheme: &str,
    previous_grapheme: &str,
) -> bool {
    let is_next_clear = !grapheme::is_alphanumeric(next_grapheme);

    let is_previous_clear = match grapheme {
        "'" => grapheme::is_whitespace(previous_grapheme),
        "\"" => !grapheme::is_alphanumeric(previous_grapheme),
        _ => true,
    };

    is_next_clear && is_previous_clear
}

pub fn handle_action(action: Action, tab: &Tab, doc: &mut Doc, ctx: &mut Ctx) -> bool {
    match action {
        action_name!(MoveLeft, mods) => handle_move(-1, 0, mods.contains(Mod::Shift), doc, ctx.gfx),
        action_name!(MoveRight, mods) => handle_move(1, 0, mods.contains(Mod::Shift), doc, ctx.gfx),
        action_name!(MoveUp, mods) => handle_move(0, -1, mods.contains(Mod::Shift), doc, ctx.gfx),
        action_name!(MoveDown, mods) => handle_move(0, 1, mods.contains(Mod::Shift), doc, ctx.gfx),
        action_name!(MoveLeftWord, mods) => {
            doc.move_cursors_to_next_word(-1, mods.contains(Mod::Shift), ctx.gfx)
        }
        action_name!(MoveRightWord, mods) => {
            doc.move_cursors_to_next_word(1, mods.contains(Mod::Shift), ctx.gfx)
        }
        action_name!(MoveUpParagraph, mods) => {
            doc.move_cursors_to_next_paragraph(-1, mods.contains(Mod::Shift), ctx.gfx)
        }
        action_name!(MoveDownParagraph, mods) => {
            doc.move_cursors_to_next_paragraph(1, mods.contains(Mod::Shift), ctx.gfx)
        }
        action_name!(ShiftLinesUp) => handle_shift_lines(-1, doc, ctx),
        action_name!(ShiftLinesDown) => handle_shift_lines(1, doc, ctx),
        action_name!(AddCursorUp) => handle_add_cursor(-1, doc, ctx.gfx),
        action_name!(AddCursorDown) => handle_add_cursor(1, doc, ctx.gfx),
        action_name!(DeleteBackward) => handle_delete_backward(DeleteKind::Char, doc, ctx),
        action_name!(DeleteBackwardWord) => handle_delete_backward(DeleteKind::Word, doc, ctx),
        action_name!(DeleteBackwardLine) => handle_delete_backward(DeleteKind::Line, doc, ctx),
        action_name!(DeleteForward) => handle_delete_forward(DeleteKind::Char, doc, ctx),
        action_name!(DeleteForwardWord) => handle_delete_forward(DeleteKind::Word, doc, ctx),
        action_keybind!(key: Enter, mods: Mods::NONE) => handle_enter(doc, ctx),
        action_keybind!(key: Tab, mods) => handle_tab(mods, doc, ctx),
        action_name!(PageUp, mods) => {
            let height_lines = tab.doc_height_lines(ctx.gfx) as isize;

            doc.move_cursors(0, -height_lines, mods.contains(Mod::Shift), ctx.gfx);
        }
        action_name!(PageDown, mods) => {
            let height_lines = tab.doc_height_lines(ctx.gfx) as isize;

            doc.move_cursors(0, height_lines, mods.contains(Mod::Shift), ctx.gfx);
        }
        action_name!(Home, mods) => handle_home(mods.contains(Mod::Shift), doc, ctx.gfx),
        action_name!(End, mods) => handle_end(mods.contains(Mod::Shift), doc, ctx.gfx),
        action_name!(GoToStart, mods) => {
            for index in doc.cursor_indices() {
                doc.jump_cursor(index, Position::ZERO, mods.contains(Mod::Shift), ctx.gfx);
            }
        }
        action_name!(GoToEnd, mods) => {
            for index in doc.cursor_indices() {
                doc.jump_cursor(index, doc.end(), mods.contains(Mod::Shift), ctx.gfx);
            }
        }
        action_name!(SelectAll) => {
            doc.jump_cursors(Position::ZERO, false, ctx.gfx);
            doc.jump_cursors(doc.end(), true, ctx.gfx);
        }
        action_keybind!(key: Escape, mods: Mods::NONE) => {
            if doc.cursors_len() > 1 {
                doc.clear_extra_cursors(CursorIndex::Some(0));
            } else {
                doc.end_cursor_selection(CursorIndex::Main);
            }
        }
        action_name!(Undo) => doc.undo(ActionKind::Done, ctx),
        action_name!(Redo) => doc.undo(ActionKind::Undone, ctx),
        action_name!(Copy) => handle_copy(doc, ctx),
        action_name!(Cut) => handle_cut(doc, ctx),
        action_name!(Paste) => handle_paste(doc, ctx),
        action_name!(AddCursorAtNextOccurance) => doc.add_cursor_at_next_occurance(ctx.gfx),
        action_name!(ToggleComments) => doc.toggle_comments_at_cursors(ctx),
        action_name!(Indent) => doc.indent_lines_at_cursors(false, ctx),
        action_name!(Unindent) => doc.indent_lines_at_cursors(true, ctx),
        action_name!(RequestCodeAction) => {
            doc.lsp_code_action(ctx);
        }
        action_name!(Rename) => {
            doc.lsp_prepare_rename(ctx);
        }
        action_name!(FindReferences) => {
            doc.lsp_references(ctx);
        }
        _ => return false,
    }

    true
}

fn handle_move(
    direction_x: isize,
    direction_y: isize,
    should_select: bool,
    doc: &mut Doc,
    gfx: &mut Gfx,
) {
    for index in doc.cursor_indices() {
        let cursor = doc.cursor(index);

        match cursor.get_selection() {
            Some(selection) if !should_select => {
                if direction_x < 0 || direction_y < 0 {
                    doc.jump_cursor(index, selection.start, false, gfx);
                } else if direction_x > 0 || direction_y > 0 {
                    doc.jump_cursor(index, selection.end, false, gfx);
                }

                if direction_y != 0 {
                    doc.move_cursor(index, direction_x, direction_y, false, gfx);
                }
            }
            _ => doc.move_cursor(index, direction_x, direction_y, should_select, gfx),
        }
    }
}

pub fn handle_add_cursor(direction_y: isize, doc: &mut Doc, gfx: &mut Gfx) {
    let cursor = doc.cursor(CursorIndex::Main);

    let position = doc.move_position_with_desired_visual_x(
        cursor.position,
        0,
        direction_y,
        Some(cursor.desired_visual_x),
        gfx,
    );

    doc.add_cursor(Cursor::new(position, cursor.desired_visual_x));
}

pub fn handle_left_click(
    doc: &mut Doc,
    position: Position,
    mods: Mods,
    count: MouseClickCount,
    is_drag: bool,
    gfx: &mut Gfx,
) {
    let do_extend_selection = is_drag || mods.contains(Mod::Shift);

    if count == MouseClickCount::Single {
        doc.jump_cursors(position, do_extend_selection, gfx);
        return;
    }

    if !do_extend_selection {
        match count {
            MouseClickCount::Double => doc.select_current_word_at_cursors(gfx),
            MouseClickCount::Triple => doc.select_current_line_at_cursors(gfx),
            _ => {}
        }

        return;
    }

    let select_at_position = if count == MouseClickCount::Double {
        Doc::select_current_word_at_position
    } else {
        Doc::select_current_line_at_position
    };

    let word_selection = select_at_position(doc, position, gfx);

    let cursor = doc.cursor(CursorIndex::Main);

    let selection_anchor = cursor.selection_anchor.unwrap_or(cursor.position);

    let is_selected_word_left_of_anchor = cursor
        .get_selection()
        .map(|selection| selection_anchor == selection.end)
        .unwrap_or(false);

    let selection_anchor_word = select_at_position(
        doc,
        if is_selected_word_left_of_anchor {
            doc.move_position(selection_anchor, -1, 0, gfx)
        } else {
            selection_anchor
        },
        gfx,
    );

    let (start, end) = if selection_anchor <= position {
        (selection_anchor_word.start, word_selection.end)
    } else {
        (selection_anchor_word.end, word_selection.start)
    };

    doc.jump_cursors(start, false, gfx);
    doc.jump_cursors(end, true, gfx);
}

pub fn handle_delete_backward(kind: DeleteKind, doc: &mut Doc, ctx: &mut Ctx) {
    for index in doc.cursor_indices() {
        let cursor = doc.cursor(index);

        let (start, end) = if let Some(selection) = cursor.get_selection() {
            (selection.start, selection.end)
        } else {
            let mut end = cursor.position;

            let start = match kind {
                DeleteKind::Char => {
                    if end.x > 0 && end.x == doc.line_start(end.y) {
                        doc.indent_start(end, ctx)
                    } else {
                        let start = doc.move_position(end, -1, 0, ctx.gfx);
                        let start_grapheme = doc.grapheme(start);

                        if get_matching_grapheme(start_grapheme)
                            == Some(doc.grapheme(cursor.position))
                        {
                            end = doc.move_position(end, 1, 0, ctx.gfx);
                        }

                        start
                    }
                }
                DeleteKind::Word => doc.move_position_to_next_word(end, -1, ctx.gfx),
                DeleteKind::Line => Position::new(0, end.y),
            };

            (start, end)
        };

        doc.delete(start, end, ctx);
        doc.end_cursor_selection(index);
    }
}

fn handle_delete_forward(kind: DeleteKind, doc: &mut Doc, ctx: &mut Ctx) {
    for index in doc.cursor_indices() {
        let cursor = doc.cursor(index);

        let (start, end) = if let Some(selection) = cursor.get_selection() {
            (selection.start, selection.end)
        } else {
            let start = cursor.position;

            let end = match kind {
                DeleteKind::Char => doc.move_position(start, 1, 0, ctx.gfx),
                DeleteKind::Word => doc.move_position_to_next_word(start, 1, ctx.gfx),
                DeleteKind::Line => doc.line_end(start.y),
            };

            (start, end)
        };

        doc.delete(start, end, ctx);
        doc.end_cursor_selection(index);
    }
}

pub fn handle_enter(doc: &mut Doc, ctx: &mut Ctx) {
    for index in doc.cursor_indices() {
        let cursor = doc.cursor(index);

        let mut indent_y = cursor.position.y;

        while indent_y > 0 && doc.is_line_whitespace(indent_y) {
            indent_y -= 1;
        }

        let indent_line = doc.get_line(indent_y).unwrap_or_default();

        let indent_end = if indent_y == cursor.position.y {
            doc.line_start(indent_y).min(cursor.position.x)
        } else {
            doc.line_start(indent_y)
        };

        let mut indent_text = STRING_POOL.new_item();
        indent_text.push_str(&indent_line[..indent_end]);

        let previous_position = doc.move_position(cursor.position, -1, 0, ctx.gfx);
        let do_start_block = doc.grapheme(previous_position) == "{";
        let do_end_block = do_start_block && doc.grapheme(cursor.position) == "}";

        doc.insert_at_cursor(index, "\n", ctx);
        doc.insert_at_cursor(index, &indent_text, ctx);

        if do_start_block {
            let do_newline_brackets = ctx
                .config
                .get_language_for_doc(doc)
                .is_some_and(|language| language.do_newline_brackets);

            if do_newline_brackets && doc.line_start(previous_position.y) != previous_position.x {
                doc.insert(previous_position, "\n", ctx);
                doc.insert_at_cursor(index, &indent_text, ctx);
                doc.trim_trailing_whitespace_at(previous_position.y, ctx);
            }

            doc.indent_at_cursor(index, ctx);
        }

        if do_end_block {
            let cursor_position = doc.cursor(index).position;

            doc.insert_at_cursor(index, "\n", ctx);
            doc.insert_at_cursor(index, &indent_text, ctx);

            doc.jump_cursor(index, cursor_position, false, ctx.gfx);
        }
    }
}

fn handle_tab(mods: Mods, doc: &mut Doc, ctx: &mut Ctx) {
    let do_unindent = mods.contains(Mod::Shift);

    for index in doc.cursor_indices() {
        let cursor = doc.cursor(index);

        if cursor.get_selection().is_some() || do_unindent {
            doc.indent_lines_at_cursor(index, do_unindent, ctx);
        } else {
            doc.indent_at_cursors(ctx);
        }
    }
}

fn handle_home(should_select: bool, doc: &mut Doc, gfx: &mut Gfx) {
    for index in doc.cursor_indices() {
        let cursor = doc.cursor(index);
        let line_start_x = doc.line_start(cursor.position.y);

        let x = if line_start_x == cursor.position.x {
            0
        } else {
            line_start_x
        };

        let position = Position::new(x, cursor.position.y);

        doc.jump_cursor(index, position, should_select, gfx);
    }
}

fn handle_end(should_select: bool, doc: &mut Doc, gfx: &mut Gfx) {
    for index in doc.cursor_indices() {
        let position = doc.line_end(doc.cursor(index).position.y);

        doc.jump_cursor(index, position, should_select, gfx);
    }
}

fn handle_cut(doc: &mut Doc, ctx: &mut Ctx) {
    let mut text = STRING_POOL.new_item();
    let was_copy_implicit = doc.copy_at_cursors(&mut text);

    let _ = ctx.window.set_clipboard(&text, was_copy_implicit);

    for index in doc.cursor_indices() {
        let cursor = doc.cursor(index);

        let selection = cursor
            .get_selection()
            .unwrap_or(doc.select_current_line_at_position(cursor.position, ctx.gfx));

        doc.delete(selection.start, selection.end, ctx);
        doc.end_cursor_selection(index);
    }
}

pub fn handle_copy(doc: &Doc, ctx: &mut Ctx) {
    let mut text = STRING_POOL.new_item();
    let was_copy_implicit = doc.copy_at_cursors(&mut text);

    let _ = ctx.window.set_clipboard(&text, was_copy_implicit);
}

fn handle_paste(doc: &mut Doc, ctx: &mut Ctx) {
    let was_copy_implicit = ctx.window.was_copy_implicit();

    let mut text = STRING_POOL.new_item();
    let _ = ctx.window.get_clipboard(&mut text);

    doc.paste_at_cursors(&text, was_copy_implicit, ctx);
}

fn handle_shift_lines(direction: isize, doc: &mut Doc, ctx: &mut Ctx) {
    let direction = direction.signum();

    for index in doc.cursor_indices() {
        let cursor = doc.cursor(index);
        let cursor_position = cursor.position;
        let cursor_selection = cursor.get_selection();

        let had_selection = cursor_selection.is_some();
        let cursor_selection = cursor_selection.unwrap_or(Selection {
            start: cursor_position,
            end: cursor_position,
        });

        match direction.cmp(&0) {
            Ordering::Less => {
                if cursor_selection.start.y == 0 {
                    continue;
                }
            }
            Ordering::Greater => {
                if cursor_selection.end.y == doc.lines().len() - 1 {
                    continue;
                }
            }
            Ordering::Equal => continue,
        };

        let mut text = STRING_POOL.new_item();

        if direction > 0 {
            text.push('\n');
        }

        let selection = cursor_selection.trim();
        let mut start = Position::new(0, selection.start.y);
        let mut end = doc.line_end(selection.end.y);

        doc.collect_string(start, end, &mut text);

        if direction < 0 {
            text.push('\n');
        }

        if end.y == doc.lines().len() - 1 {
            start = doc.move_position(start, -1, 0, ctx.gfx);
        } else {
            end = doc.move_position(end, 1, 0, ctx.gfx);
        }

        doc.delete(start, end, ctx);

        let insert_start = if direction < 0 {
            Position::new(0, selection.start.y - 1)
        } else {
            doc.line_end(selection.start.y)
        };

        doc.insert(insert_start, &text, ctx);

        // Reset the selection to prevent it being expanded by the latest insert.
        if had_selection {
            let new_selection_start = Position::new(
                cursor_selection.start.x,
                cursor_selection.start.y.saturating_add_signed(direction),
            );
            let new_selection_end = Position::new(
                cursor_selection.end.x,
                cursor_selection.end.y.saturating_add_signed(direction),
            );

            if cursor_position == cursor_selection.start {
                doc.jump_cursor(index, new_selection_end, false, ctx.gfx);
                doc.jump_cursor(index, new_selection_start, true, ctx.gfx);
            } else {
                doc.jump_cursor(index, new_selection_start, false, ctx.gfx);
                doc.jump_cursor(index, new_selection_end, true, ctx.gfx);
            }
        } else {
            let new_position = Position::new(
                cursor_position.x,
                cursor_position.y.saturating_add_signed(direction),
            );

            doc.jump_cursor(index, new_position, false, ctx.gfx);
        }
    }
}

fn get_matching_grapheme(grapheme: &str) -> Option<&str> {
    match grapheme {
        "\"" => Some("\""),
        "'" => Some("'"),
        "(" => Some(")"),
        "[" => Some("]"),
        "{" => Some("}"),
        _ => None,
    }
}

fn is_matching_grapheme(grapheme: &str) -> bool {
    matches!(grapheme, "\"" | "'" | ")" | "]" | "}")
}
