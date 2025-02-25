use std::cmp::Ordering;

use crate::{
    config::language::Language,
    editor_buffers::EditorBuffers,
    geometry::position::Position,
    platform::window::Window,
    temp_buffer::TempBuffer,
    text::{
        action_history::ActionKind, cursor_index::CursorIndex, doc::Doc, line_pool::LinePool,
        selection::Selection,
    },
};

use super::{
    action::{action_keybind, action_name, Action},
    keybind::MOD_SHIFT,
    mousebind::MouseClickKind,
};

enum DeleteKind {
    Char,
    Word,
    Line,
}

pub fn handle_char(c: char, doc: &mut Doc, line_pool: &mut LinePool, time: f32) {
    for index in doc.cursor_indices() {
        let cursor = doc.get_cursor(index);

        let current_char = doc.get_char(cursor.position);

        let previous_position = doc.move_position(cursor.position, Position::new(-1, 0));
        let previous_char = doc.get_char(previous_position);

        if is_matching_char(c) && c == current_char {
            doc.move_cursor(index, Position::new(1, 0), false);

            continue;
        }

        if let Some(matching_char) = get_matching_char(c).filter(|c| {
            (current_char == *c || current_char.is_whitespace())
                && (*c != '\'' || previous_char.is_whitespace())
        }) {
            doc.insert_at_cursor(index, &[c, matching_char], line_pool, time);
            doc.move_cursor(index, Position::new(-1, 0), false);

            continue;
        }

        doc.insert_at_cursor(index, &[c], line_pool, time);
    }
}

pub fn handle_action(
    action: Action,
    window: &mut Window,
    doc: &mut Doc,
    language: Option<&Language>,
    buffers: &mut EditorBuffers,
    time: f32,
) -> bool {
    match action {
        action_name!(MoveLeft, mods) => {
            handle_move(Position::new(-1, 0), mods & MOD_SHIFT != 0, doc)
        }
        action_name!(MoveRight, mods) => {
            handle_move(Position::new(1, 0), mods & MOD_SHIFT != 0, doc)
        }
        action_name!(MoveUp, mods) => handle_move(Position::new(0, -1), mods & MOD_SHIFT != 0, doc),
        action_name!(MoveDown, mods) => {
            handle_move(Position::new(0, 1), mods & MOD_SHIFT != 0, doc)
        }
        action_name!(MoveLeftWord, mods) => {
            doc.move_cursors_to_next_word(-1, mods & MOD_SHIFT != 0)
        }
        action_name!(MoveRightWord, mods) => {
            doc.move_cursors_to_next_word(1, mods & MOD_SHIFT != 0)
        }
        action_name!(MoveUpParagraph, mods) => {
            doc.move_cursors_to_next_paragraph(-1, mods & MOD_SHIFT != 0)
        }
        action_name!(MoveDownParagraph, mods) => {
            doc.move_cursors_to_next_paragraph(1, mods & MOD_SHIFT != 0)
        }
        action_name!(ShiftLinesUp) => handle_shift_lines(-1, doc, buffers, time),
        action_name!(ShiftLinesDown) => handle_shift_lines(1, doc, buffers, time),
        action_name!(UndoCursorPosition) => doc.undo_cursor_position(),
        action_name!(RedoCursorPosition) => doc.redo_cursor_position(),
        action_name!(AddCursorUp) => handle_add_cursor(Position::new(0, -1), doc),
        action_name!(AddCursorDown) => handle_add_cursor(Position::new(0, 1), doc),
        action_name!(DeleteBackward) => {
            handle_delete_backward(DeleteKind::Char, doc, language, &mut buffers.lines, time)
        }
        action_name!(DeleteBackwardWord) => {
            handle_delete_backward(DeleteKind::Word, doc, language, &mut buffers.lines, time)
        }
        action_name!(DeleteBackwardLine) => {
            handle_delete_backward(DeleteKind::Line, doc, language, &mut buffers.lines, time)
        }
        action_name!(DeleteForward) => {
            handle_delete_forward(DeleteKind::Char, doc, &mut buffers.lines, time)
        }
        action_name!(DeleteForwardWord) => {
            handle_delete_forward(DeleteKind::Word, doc, &mut buffers.lines, time)
        }
        action_keybind!(key: Enter, mods: 0) => {
            handle_enter(doc, language, buffers, time);
        }
        action_keybind!(key: Tab, mods) => {
            handle_tab(mods, doc, language, &mut buffers.lines, time);
        }
        action_name!(PageUp, mods) => {
            let height_lines = window.gfx().height_lines();

            doc.move_cursors(Position::new(0, -height_lines), mods & MOD_SHIFT != 0);
        }
        action_name!(PageDown, mods) => {
            let height_lines = window.gfx().height_lines();

            doc.move_cursors(Position::new(0, height_lines), mods & MOD_SHIFT != 0);
        }
        action_name!(Home, mods) => {
            handle_home(mods & MOD_SHIFT != 0, doc);
        }
        action_name!(End, mods) => {
            handle_end(mods & MOD_SHIFT != 0, doc);
        }
        action_name!(GoToStart, mods) => {
            for index in doc.cursor_indices() {
                doc.jump_cursor(index, Position::zero(), mods & MOD_SHIFT != 0);
            }
        }
        action_name!(GoToEnd, mods) => {
            for index in doc.cursor_indices() {
                doc.jump_cursor(index, doc.end(), mods & MOD_SHIFT != 0);
            }
        }
        action_name!(SelectAll) => {
            let y = doc.lines().len() as isize - 1;
            let x = doc.get_line_len(y);

            doc.jump_cursors(Position::zero(), false);
            doc.jump_cursors(Position::new(x, y), true);
        }
        action_keybind!(key: Escape, mods: 0) => {
            if doc.cursors_len() > 1 {
                doc.clear_extra_cursors(CursorIndex::Some(0));
            } else {
                doc.end_cursor_selection(CursorIndex::Main);
            }
        }
        action_name!(Undo) => {
            doc.undo(&mut buffers.lines, ActionKind::Done);
        }
        action_name!(Redo) => {
            doc.undo(&mut buffers.lines, ActionKind::Undone);
        }
        action_name!(Copy) => {
            handle_copy(window, doc, &mut buffers.text);
        }
        action_name!(Cut) => {
            handle_cut(window, doc, buffers, time);
        }
        action_name!(Paste) => {
            handle_paste(window, doc, &mut buffers.lines, time);
        }
        action_name!(AddCursorAtNextOccurance) => {
            doc.add_cursor_at_next_occurance();
        }
        action_name!(ToggleComments) => {
            if let Some(language) = language {
                doc.toggle_comments_at_cursors(&language.comment, &mut buffers.lines, time);
            }
        }
        action_name!(Indent) => {
            let indent_width = language
                .map(|language| language.indent_width)
                .unwrap_or_default();

            doc.indent_lines_at_cursors(indent_width, false, &mut buffers.lines, time);
        }
        action_name!(Unindent) => {
            let indent_width = language
                .map(|language| language.indent_width)
                .unwrap_or_default();

            doc.indent_lines_at_cursors(indent_width, true, &mut buffers.lines, time);
        }
        _ => return false,
    }

    true
}

fn handle_move(direction: Position, should_select: bool, doc: &mut Doc) {
    for index in doc.cursor_indices() {
        let cursor = doc.get_cursor(index);

        match cursor.get_selection() {
            Some(selection) if !should_select => {
                if direction.x < 0 || direction.y < 0 {
                    doc.jump_cursor(index, selection.start, false);
                } else if direction.x > 0 || direction.y > 0 {
                    doc.jump_cursor(index, selection.end, false);
                }

                if direction.y != 0 {
                    doc.move_cursor(index, direction, false);
                }
            }
            _ => doc.move_cursor(index, direction, should_select),
        }
    }
}

fn handle_add_cursor(direction: Position, doc: &mut Doc) {
    let cursor = doc.get_cursor(CursorIndex::Main);

    let position = doc.move_position_with_desired_visual_x(
        cursor.position,
        direction,
        Some(cursor.desired_visual_x),
    );

    doc.add_cursor(position);
}

pub fn handle_left_click(
    doc: &mut Doc,
    position: Position,
    mods: u8,
    kind: MouseClickKind,
    is_drag: bool,
) {
    let do_extend_selection = is_drag || (mods & MOD_SHIFT != 0);

    if kind == MouseClickKind::Single {
        doc.jump_cursors(position, do_extend_selection);
        return;
    }

    if !do_extend_selection {
        match kind {
            MouseClickKind::Double => doc.select_current_word_at_cursors(),
            MouseClickKind::Triple => doc.select_current_line_at_cursors(),
            _ => {}
        }

        return;
    }

    let select_at_position = if kind == MouseClickKind::Double {
        Doc::select_current_word_at_position
    } else {
        Doc::select_current_line_at_position
    };

    let word_selection = select_at_position(doc, position);

    let cursor = doc.get_cursor(CursorIndex::Main);

    let selection_anchor = cursor.selection_anchor.unwrap_or(cursor.position);

    let is_selected_word_left_of_anchor = cursor
        .get_selection()
        .map(|selection| selection_anchor == selection.end)
        .unwrap_or(false);

    let selection_anchor_word = select_at_position(
        doc,
        if is_selected_word_left_of_anchor {
            doc.move_position(selection_anchor, Position::new(-1, 0))
        } else {
            selection_anchor
        },
    );

    let (start, end) = if selection_anchor <= position {
        (selection_anchor_word.start, word_selection.end)
    } else {
        (selection_anchor_word.end, word_selection.start)
    };

    doc.jump_cursors(start, false);
    doc.jump_cursors(end, true);
}

fn handle_delete_backward(
    kind: DeleteKind,
    doc: &mut Doc,
    language: Option<&Language>,
    line_pool: &mut LinePool,
    time: f32,
) {
    let indent_width = language
        .map(|language| language.indent_width)
        .unwrap_or_default();

    for index in doc.cursor_indices() {
        let cursor = doc.get_cursor(index);

        let (start, end) = if let Some(selection) = cursor.get_selection() {
            (selection.start, selection.end)
        } else {
            let mut end = cursor.position;

            let start = match kind {
                DeleteKind::Char => {
                    if end.x > 0 && end.x == doc.get_line_start(end.y) {
                        doc.get_indent_start(end, indent_width)
                    } else {
                        let start = doc.move_position(end, Position::new(-1, 0));
                        let start_char = doc.get_char(start);

                        if get_matching_char(start_char) == Some(doc.get_char(cursor.position)) {
                            end = doc.move_position(end, Position::new(1, 0));
                        }

                        start
                    }
                }
                DeleteKind::Word => doc.move_position_to_next_word(end, -1),
                DeleteKind::Line => Position::new(0, end.y),
            };

            (start, end)
        };

        doc.delete(start, end, line_pool, time);
        doc.end_cursor_selection(index);
    }
}

fn handle_delete_forward(kind: DeleteKind, doc: &mut Doc, line_pool: &mut LinePool, time: f32) {
    for index in doc.cursor_indices() {
        let cursor = doc.get_cursor(index);

        let (start, end) = if let Some(selection) = cursor.get_selection() {
            (selection.start, selection.end)
        } else {
            let start = cursor.position;

            let end = match kind {
                DeleteKind::Char => doc.move_position(start, Position::new(1, 0)),
                DeleteKind::Word => doc.move_position_to_next_word(start, 1),
                DeleteKind::Line => Position::new(doc.get_line_len(start.y), start.y),
            };

            (start, end)
        };

        doc.delete(start, end, line_pool, time);
        doc.end_cursor_selection(index);
    }
}

fn handle_enter(
    doc: &mut Doc,
    language: Option<&Language>,
    buffers: &mut EditorBuffers,
    time: f32,
) {
    let text_buffer = buffers.text.get_mut();

    for index in doc.cursor_indices() {
        let cursor = doc.get_cursor(index);

        let mut indent_position = Position::new(0, cursor.position.y);

        while indent_position < cursor.position {
            let c = doc.get_char(indent_position);

            if c.is_whitespace() {
                text_buffer.push(c);
                indent_position = doc.move_position(indent_position, Position::new(1, 0));
            } else {
                break;
            }
        }

        let previous_position = doc.move_position(cursor.position, Position::new(-1, 0));
        let do_start_block =
            doc.get_char(previous_position) == '{' && doc.get_char(cursor.position) == '}';

        doc.insert_at_cursor(index, &['\n'], &mut buffers.lines, time);
        doc.insert_at_cursor(index, text_buffer, &mut buffers.lines, time);

        if do_start_block {
            let indent_width = language
                .map(|language| language.indent_width)
                .unwrap_or_default();

            doc.indent_at_cursor(index, indent_width, &mut buffers.lines, time);

            let cursor_position = doc.get_cursor(index).position;

            doc.insert_at_cursor(index, &['\n'], &mut buffers.lines, time);
            doc.insert_at_cursor(index, text_buffer, &mut buffers.lines, time);

            doc.jump_cursor(index, cursor_position, false);
        }
    }
}

fn handle_tab(
    mods: u8,
    doc: &mut Doc,
    language: Option<&Language>,
    line_pool: &mut LinePool,
    time: f32,
) {
    let indent_width = language
        .map(|language| language.indent_width)
        .unwrap_or_default();
    let do_unindent = mods & MOD_SHIFT != 0;

    for index in doc.cursor_indices() {
        let cursor = doc.get_cursor(index);

        if cursor.get_selection().is_some() || do_unindent {
            doc.indent_lines_at_cursor(index, indent_width, do_unindent, line_pool, time);
        } else {
            doc.indent_at_cursors(indent_width, line_pool, time);
        }
    }
}

fn handle_home(should_select: bool, doc: &mut Doc) {
    for index in doc.cursor_indices() {
        let cursor = doc.get_cursor(index);
        let line_start_x = doc.get_line_start(cursor.position.y);

        let x = if line_start_x == cursor.position.x {
            0
        } else {
            line_start_x
        };

        let position = Position::new(x, cursor.position.y);

        doc.jump_cursor(index, position, should_select);
    }
}

fn handle_end(should_select: bool, doc: &mut Doc) {
    for index in doc.cursor_indices() {
        let mut position = doc.get_cursor(index).position;
        position.x = doc.get_line_len(position.y);

        doc.jump_cursor(index, position, should_select);
    }
}

fn handle_cut(window: &mut Window, doc: &mut Doc, buffers: &mut EditorBuffers, time: f32) {
    let text_buffer = buffers.text.get_mut();
    let was_copy_implicit = doc.copy_at_cursors(text_buffer);

    let _ = window.set_clipboard(text_buffer, was_copy_implicit);

    for index in doc.cursor_indices() {
        let cursor = doc.get_cursor(index);

        let selection = cursor
            .get_selection()
            .unwrap_or(doc.select_current_line_at_position(cursor.position));

        doc.delete(selection.start, selection.end, &mut buffers.lines, time);
        doc.end_cursor_selection(index);
    }
}

pub fn handle_copy(window: &mut Window, doc: &mut Doc, text_buffer: &mut TempBuffer<char>) {
    let text_buffer = text_buffer.get_mut();
    let was_copy_implicit = doc.copy_at_cursors(text_buffer);

    let _ = window.set_clipboard(text_buffer, was_copy_implicit);
}

fn handle_paste(window: &mut Window, doc: &mut Doc, line_pool: &mut LinePool, time: f32) {
    let was_copy_implicit = window.was_copy_implicit();
    let text = window.get_clipboard().unwrap_or(&[]);

    doc.paste_at_cursors(text, was_copy_implicit, line_pool, time);
}

fn handle_shift_lines(direction: isize, doc: &mut Doc, buffers: &mut EditorBuffers, time: f32) {
    let direction = direction.signum();

    for index in doc.cursor_indices() {
        let cursor = doc.get_cursor(index);
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
                if cursor_selection.end.y == doc.lines().len() as isize - 1 {
                    continue;
                }
            }
            Ordering::Equal => continue,
        };

        let selection = cursor_selection.trim_lines_without_selected_chars();

        let text_buffer = buffers.text.get_mut();

        let mut start = Position::new(0, selection.start.y);
        let mut end = Position::new(doc.get_line_len(selection.end.y), selection.end.y);

        if direction > 0 {
            text_buffer.push('\n');
        }

        doc.collect_chars(start, end, text_buffer);

        if direction < 0 {
            text_buffer.push('\n');
        }

        if end.y as usize == doc.lines().len() - 1 {
            start = doc.move_position(start, Position::new(-1, 0));
        } else {
            end = doc.move_position(end, Position::new(1, 0));
        }

        doc.delete(start, end, &mut buffers.lines, time);

        let insert_start = if direction < 0 {
            Position::new(0, selection.start.y - 1)
        } else {
            Position::new(doc.get_line_len(selection.start.y), selection.start.y)
        };

        doc.insert(insert_start, text_buffer.iter(), &mut buffers.lines, time);

        // Reset the selection to prevent it being expanded by the latest insert.
        if had_selection {
            let new_selection_start = Position::new(
                cursor_selection.start.x,
                cursor_selection.start.y + direction,
            );
            let new_selection_end =
                Position::new(cursor_selection.end.x, cursor_selection.end.y + direction);

            if cursor_position == cursor_selection.start {
                doc.jump_cursor(index, new_selection_end, false);
                doc.jump_cursor(index, new_selection_start, true);
            } else {
                doc.jump_cursor(index, new_selection_start, false);
                doc.jump_cursor(index, new_selection_end, true);
            }
        } else {
            let new_position = Position::new(cursor_position.x, cursor_position.y + direction);

            doc.jump_cursor(index, new_position, false);
        }
    }
}

fn get_matching_char(c: char) -> Option<char> {
    match c {
        '"' => Some('"'),
        '\'' => Some('\''),
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}

fn is_matching_char(c: char) -> bool {
    matches!(c, '"' | '\'' | ')' | ']' | '}')
}
