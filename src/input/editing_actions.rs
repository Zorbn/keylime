use std::cmp::Ordering;

use crate::{
    config::language::Language,
    geometry::position::Position,
    platform::window::Window,
    temp_buffer::TempBuffer,
    text::{
        action_history::ActionKind, cursor_index::CursorIndex, doc::Doc, line_pool::LinePool,
        selection::Selection,
    },
};

use super::{
    key::Key,
    keybind::{Keybind, MOD_ALT, MOD_CTRL, MOD_SHIFT},
    mousebind::MouseClickKind,
};

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

pub fn handle_keybind(
    keybind: Keybind,
    window: &mut Window,
    doc: &mut Doc,
    language: Option<&Language>,
    line_pool: &mut LinePool,
    text_buffer: &mut TempBuffer<char>,
    time: f32,
) -> bool {
    match keybind {
        Keybind {
            key: Key::Up | Key::Down | Key::Left | Key::Right,
            mods,
        } => {
            let key = keybind.key;
            handle_arrow(key, mods, doc, line_pool, text_buffer, time);
        }
        Keybind {
            key: Key::Backspace,
            mods,
        } => {
            handle_backspace(mods, doc, language, line_pool, time);
        }
        Keybind {
            key: Key::Delete,
            mods,
        } => {
            handle_delete(mods, doc, line_pool, time);
        }
        Keybind {
            key: Key::Enter,
            mods: 0,
        } => {
            handle_enter(doc, language, line_pool, text_buffer, time);
        }
        Keybind {
            key: Key::Tab,
            mods,
        } => {
            handle_tab(mods, doc, language, line_pool, time);
        }
        Keybind {
            key: Key::PageUp,
            mods: 0 | MOD_SHIFT,
        } => {
            let mods = keybind.mods;
            let height_lines = window.gfx().height_lines();

            doc.move_cursors(Position::new(0, -height_lines), mods & MOD_SHIFT != 0);
        }
        Keybind {
            key: Key::PageDown,
            mods: 0 | MOD_SHIFT,
        } => {
            let mods = keybind.mods;
            let height_lines = window.gfx().height_lines();

            doc.move_cursors(Position::new(0, height_lines), mods & MOD_SHIFT != 0);
        }
        Keybind {
            key: Key::Home,
            mods,
        } => {
            handle_home(mods, doc);
        }
        Keybind {
            key: Key::End,
            mods,
        } => {
            handle_end(mods, doc);
        }
        Keybind {
            key: Key::A,
            mods: MOD_CTRL,
        } => {
            let y = doc.lines().len() as isize - 1;
            let x = doc.get_line_len(y);

            doc.jump_cursors(Position::zero(), false);
            doc.jump_cursors(Position::new(x, y), true);
        }
        Keybind {
            key: Key::Escape,
            mods: 0,
        } => {
            if doc.cursors_len() > 1 {
                doc.clear_extra_cursors(CursorIndex::Some(0));
            } else {
                doc.end_cursor_selection(CursorIndex::Main);
            }
        }
        Keybind {
            key: Key::Z,
            mods: MOD_CTRL,
        } => {
            doc.undo(line_pool, ActionKind::Done);
        }
        Keybind {
            key: Key::Y,
            mods: MOD_CTRL,
        } => {
            doc.undo(line_pool, ActionKind::Undone);
        }
        Keybind {
            key: Key::C,
            mods: MOD_CTRL,
        } => {
            handle_copy(window, doc, text_buffer);
        }
        Keybind {
            key: Key::X,
            mods: MOD_CTRL,
        } => {
            handle_cut(window, doc, line_pool, text_buffer, time);
        }
        Keybind {
            key: Key::V,
            mods: MOD_CTRL,
        } => {
            handle_paste(window, doc, line_pool, time);
        }
        Keybind {
            key: Key::D,
            mods: MOD_CTRL,
        } => {
            doc.add_cursor_at_next_occurance();
        }
        Keybind {
            key: Key::ForwardSlash,
            mods: MOD_CTRL,
        } => {
            if let Some(language) = language {
                doc.toggle_comments_at_cursors(&language.comment, line_pool, time);
            }
        }
        Keybind {
            key: Key::LBracket | Key::RBracket,
            mods: MOD_CTRL,
        } => {
            let indent_width = language
                .map(|language| language.indent_width)
                .unwrap_or_default();
            let do_unindent = keybind.key == Key::LBracket;

            doc.indent_lines_at_cursors(indent_width, do_unindent, line_pool, time);
        }
        _ => return false,
    }

    true
}

fn handle_arrow(
    key: Key,
    mods: u8,
    doc: &mut Doc,
    line_pool: &mut LinePool,
    text_buffer: &mut TempBuffer<char>,
    time: f32,
) {
    let direction = match key {
        Key::Up => Position::new(0, -1),
        Key::Down => Position::new(0, 1),
        Key::Left => Position::new(-1, 0),
        Key::Right => Position::new(1, 0),
        _ => unreachable!(),
    };

    let should_select = mods & MOD_SHIFT != 0;

    if (mods & MOD_CTRL != 0) && (mods & MOD_ALT != 0) && matches!(key, Key::Up | Key::Down) {
        let cursor = doc.get_cursor(CursorIndex::Main);

        let position = doc.move_position_with_desired_visual_x(
            cursor.position,
            direction,
            Some(cursor.desired_visual_x),
        );

        doc.add_cursor(position);
    } else if mods & MOD_CTRL != 0 {
        match key {
            Key::Up => doc.move_cursors_to_next_paragraph(-1, should_select),
            Key::Down => doc.move_cursors_to_next_paragraph(1, should_select),
            Key::Left => doc.move_cursors_to_next_word(-1, should_select),
            Key::Right => doc.move_cursors_to_next_word(1, should_select),
            _ => unreachable!(),
        }
    } else if mods & MOD_ALT != 0 {
        match key {
            Key::Up => handle_shift_lines(-1, doc, line_pool, text_buffer, time),
            Key::Down => handle_shift_lines(1, doc, line_pool, text_buffer, time),
            Key::Left => doc.undo_cursor_position(),
            Key::Right => doc.redo_cursor_position(),
            _ => unreachable!(),
        }
    } else {
        for index in doc.cursor_indices() {
            let cursor = doc.get_cursor(index);

            match cursor.get_selection() {
                Some(selection) if !should_select => {
                    if matches!(key, Key::Left | Key::Up) {
                        doc.jump_cursor(index, selection.start, false);
                    } else if matches!(key, Key::Right | Key::Down) {
                        doc.jump_cursor(index, selection.end, false);
                    }

                    if matches!(key, Key::Up | Key::Down) {
                        doc.move_cursor(index, direction, false);
                    }
                }
                _ => doc.move_cursor(index, direction, should_select),
            }
        }
    }
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

fn handle_backspace(
    mods: u8,
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

            let start = if mods & MOD_CTRL != 0 {
                doc.move_position_to_next_word(end, -1)
            } else if end.x > 0 && end.x == doc.get_line_start(end.y) {
                doc.get_indent_start(end, indent_width)
            } else {
                let start = doc.move_position(end, Position::new(-1, 0));
                let start_char = doc.get_char(start);

                if get_matching_char(start_char) == Some(doc.get_char(cursor.position)) {
                    end = doc.move_position(end, Position::new(1, 0));
                }

                start
            };

            (start, end)
        };

        doc.delete(start, end, line_pool, time);
        doc.end_cursor_selection(index);
    }
}

fn handle_delete(mods: u8, doc: &mut Doc, line_pool: &mut LinePool, time: f32) {
    for index in doc.cursor_indices() {
        let cursor = doc.get_cursor(index);

        let (start, end) = if let Some(selection) = cursor.get_selection() {
            (selection.start, selection.end)
        } else {
            let start = cursor.position;

            let end = if mods & MOD_CTRL != 0 {
                doc.move_position_to_next_word(start, 1)
            } else {
                doc.move_position(start, Position::new(1, 0))
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
    line_pool: &mut LinePool,
    text_buffer: &mut TempBuffer<char>,
    time: f32,
) {
    let text_buffer = text_buffer.get_mut();

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

        doc.insert_at_cursor(index, &['\n'], line_pool, time);
        doc.insert_at_cursor(index, text_buffer, line_pool, time);

        if do_start_block {
            let indent_width = language
                .map(|language| language.indent_width)
                .unwrap_or_default();

            doc.indent_at_cursor(index, indent_width, line_pool, time);

            let cursor_position = doc.get_cursor(index).position;

            doc.insert_at_cursor(index, &['\n'], line_pool, time);
            doc.insert_at_cursor(index, text_buffer, line_pool, time);

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

fn handle_home(mods: u8, doc: &mut Doc) {
    for index in doc.cursor_indices() {
        let position = if mods & MOD_CTRL != 0 {
            Position::new(0, 0)
        } else {
            let cursor = doc.get_cursor(index);
            let line_start_x = doc.get_line_start(cursor.position.y);

            let x = if line_start_x == cursor.position.x {
                0
            } else {
                line_start_x
            };

            Position::new(x, cursor.position.y)
        };

        doc.jump_cursor(index, position, mods & MOD_SHIFT != 0);
    }
}

fn handle_end(mods: u8, doc: &mut Doc) {
    for index in doc.cursor_indices() {
        let position = if mods & MOD_CTRL != 0 {
            doc.end()
        } else {
            let mut position = doc.get_cursor(index).position;
            position.x = doc.get_line_len(position.y);

            position
        };

        doc.jump_cursor(index, position, mods & MOD_SHIFT != 0);
    }
}

fn handle_cut(
    window: &mut Window,
    doc: &mut Doc,
    line_pool: &mut LinePool,
    text_buffer: &mut TempBuffer<char>,
    time: f32,
) {
    let text_buffer = text_buffer.get_mut();
    let was_copy_implicit = doc.copy_at_cursors(text_buffer);

    let _ = window.set_clipboard(text_buffer, was_copy_implicit);

    for index in doc.cursor_indices() {
        let cursor = doc.get_cursor(index);

        let selection = cursor
            .get_selection()
            .unwrap_or(doc.select_current_line_at_position(cursor.position));

        doc.delete(selection.start, selection.end, line_pool, time);
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

fn handle_shift_lines(
    direction: isize,
    doc: &mut Doc,
    line_pool: &mut LinePool,
    text_buffer: &mut TempBuffer<char>,
    time: f32,
) {
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

        let text_buffer = text_buffer.get_mut();

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

        doc.delete(start, end, line_pool, time);

        let insert_start = if direction < 0 {
            Position::new(0, selection.start.y - 1)
        } else {
            Position::new(doc.get_line_len(selection.start.y), selection.start.y)
        };

        doc.insert(insert_start, text_buffer.iter(), line_pool, time);

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
