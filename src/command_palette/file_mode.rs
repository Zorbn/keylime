use std::{
    fs::read_dir,
    path::{Path, PathBuf},
};

use crate::{cursor_index::CursorIndex, editor::Editor, line_pool::LinePool, position::Position};

use super::{mode::CommandPaletteMode, CommandPalette, CommandPaletteAction};

pub const MODE_OPEN_FILE: &CommandPaletteMode = &CommandPaletteMode {
    title: "Find File",
    on_open: on_open_file,
    on_submit: on_submit_open_file,
    on_complete_result: on_complete_results_file,
    on_update_results: on_update_results_file,
    on_backspace: on_backspace_file,
    ..CommandPaletteMode::default()
};

fn on_open_file(
    command_palette: &mut CommandPalette,
    editor: &mut Editor,
    line_pool: &mut LinePool,
    time: f32,
) {
    let focused_tab_index = editor.focused_tab_index();

    let Some((_, doc)) = editor.get_tab_with_doc(focused_tab_index) else {
        return;
    };

    let Some(path) = doc.path().and_then(|path| path.parent()) else {
        return;
    };

    let command_doc = &mut command_palette.doc;

    for component in path.components() {
        let Some(str) = component.as_os_str().to_str() else {
            continue;
        };

        for c in str.chars() {
            let position = command_doc.end();
            command_doc.insert(position, &[c], line_pool, time);
        }

        let position = command_doc.end();
        command_doc.insert(position, &['/'], line_pool, time);
    }
}

fn on_submit_open_file(
    command_palette: &mut CommandPalette,
    _: bool,
    editor: &mut Editor,
    line_pool: &mut LinePool,
    _: f32,
) -> CommandPaletteAction {
    if editor
        .open_file(Path::new(&command_palette.doc.to_string()), line_pool)
        .is_ok()
    {
        CommandPaletteAction::Close
    } else {
        CommandPaletteAction::Stay
    }
}

fn on_complete_results_file(
    command_palette: &mut CommandPalette,
    line_pool: &mut LinePool,
    time: f32,
) {
    if let Some(result) = command_palette
        .results
        .get(command_palette.selected_result_index)
    {
        let line_len = command_palette.doc.get_line_len(0);
        let end = Position::new(line_len, 0);
        let start = find_path_component_start(command_palette, end);

        command_palette.doc.delete(start, end, line_pool, time);

        let line_len = command_palette.doc.get_line_len(0);
        let mut start = Position::new(line_len, 0);

        for c in result.chars() {
            command_palette.doc.insert(start, &[c], line_pool, time);
            start = command_palette
                .doc
                .move_position(start, Position::new(1, 0));
        }
    }
}

fn on_update_results_file(
    command_palette: &mut CommandPalette,
    line_pool: &mut LinePool,
    time: f32,
) {
    let mut path = PathBuf::new();
    path.push(".");
    path.push(command_palette.doc.to_string());

    let dir = if path.is_dir() {
        let line_len = command_palette.doc.get_line_len(0);
        let last_char = command_palette.doc.get_char(Position::new(line_len - 1, 0));

        if line_len > 0 && !matches!(last_char, '/' | '\\' | '.') {
            command_palette
                .doc
                .insert(Position::new(line_len, 0), &['/'], line_pool, time);
        }

        path.as_path()
    } else {
        path.parent().unwrap_or(Path::new("."))
    };

    let Ok(entries) = read_dir(dir) else {
        return;
    };

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };

        let entry_path = entry.path();

        if does_path_match_prefix(&path, &entry_path) {
            if let Some(result) = entry_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|str| str.to_owned())
            {
                command_palette.results.push(result);
            }
        }
    }
}

fn on_backspace_file(
    command_palette: &mut CommandPalette,
    line_pool: &mut LinePool,
    time: f32,
) -> bool {
    let cursor = command_palette.doc.get_cursor(CursorIndex::Main);
    let end = cursor.position;
    let mut start = command_palette.doc.move_position(end, Position::new(-1, 0));

    if matches!(command_palette.doc.get_char(start), '/' | '\\') {
        start = find_path_component_start(command_palette, start);

        command_palette.doc.delete(start, end, line_pool, time);

        true
    } else {
        false
    }
}

fn find_path_component_start(command_palette: &CommandPalette, position: Position) -> Position {
    let mut start = position;

    while start > Position::zero() {
        let next_start = command_palette
            .doc
            .move_position(start, Position::new(-1, 0));

        if matches!(command_palette.doc.get_char(next_start), '/' | '\\') {
            break;
        }

        start = next_start;
    }

    start
}

fn does_path_match_prefix(prefix: &Path, path: &Path) -> bool {
    for (prefix_component, path_component) in prefix.components().zip(path.components()) {
        let Some(prefix_component) = prefix_component.as_os_str().to_str() else {
            continue;
        };

        let Some(path_component) = path_component.as_os_str().to_str() else {
            continue;
        };

        for (prefix_char, path_char) in prefix_component.chars().zip(path_component.chars()) {
            if prefix_char.to_ascii_lowercase() != path_char.to_ascii_lowercase() {
                return false;
            }
        }
    }

    true
}
