use std::{
    env::current_dir,
    fs::{read_dir, File},
    path::{Path, PathBuf},
};

use crate::{geometry::position::Position, text::cursor_index::CursorIndex};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction,
};

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
    CommandPaletteEventArgs {
        command_palette,
        pane,
        doc_list,
        line_pool,
        time,
        ..
    }: CommandPaletteEventArgs,
) {
    let focused_tab_index = pane.focused_tab_index();

    let Some((_, doc)) = pane.get_tab_with_data(focused_tab_index, doc_list) else {
        return;
    };

    let Ok(current_dir) = current_dir() else {
        return;
    };

    let Some(path) = doc
        .path()
        .and_then(|path| path.parent())
        .map(|path| path.strip_prefix(current_dir).unwrap_or(path))
    else {
        return;
    };

    let command_doc = &mut command_palette.doc;

    for component in path.components() {
        let Some(str) = component.as_os_str().to_str() else {
            continue;
        };

        for c in str.chars() {
            let position = command_doc.end();
            command_doc.insert(position, [c], line_pool, time);
        }

        let position = command_doc.end();
        command_doc.insert(position, ['/'], line_pool, time);
    }
}

fn on_submit_open_file(
    CommandPaletteEventArgs {
        command_palette,
        pane,
        doc_list,
        config,
        line_pool,
        time,
        ..
    }: CommandPaletteEventArgs,
    _: bool,
) -> CommandPaletteAction {
    let string = command_palette.doc.to_string();
    // Trim trailing whitespace, this allows entering "/path/to/file " to create "file"
    // when just "/path/to/file" could auto-complete to another result like "/path/to/filewithlongername"
    let string = string.trim_end();

    let path = Path::new(&string);

    if !path.exists() {
        let _ = File::create(path);
    }

    if pane
        .open_file(path, doc_list, config, line_pool, time)
        .is_ok()
    {
        CommandPaletteAction::Close
    } else {
        CommandPaletteAction::Stay
    }
}

fn on_complete_results_file(
    CommandPaletteEventArgs {
        command_palette,
        line_pool,
        time,
        ..
    }: CommandPaletteEventArgs,
) {
    let Some(result) = command_palette.result_list.get_selected_result() else {
        return;
    };

    let line_len = command_palette.doc.get_line_len(0);
    let end = Position::new(line_len, 0);
    let start = find_path_component_start(command_palette, end);

    command_palette.doc.delete(start, end, line_pool, time);

    let line_len = command_palette.doc.get_line_len(0);
    let mut start = Position::new(line_len, 0);

    for c in result.chars() {
        command_palette.doc.insert(start, [c], line_pool, time);
        start = command_palette
            .doc
            .move_position(start, Position::new(1, 0));
    }
}

fn on_update_results_file(
    CommandPaletteEventArgs {
        command_palette, ..
    }: CommandPaletteEventArgs,
) {
    let mut path = PathBuf::new();
    let dir = get_command_palette_dir(command_palette, &mut path);

    let Ok(entries) = read_dir(dir) else {
        return;
    };

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };

        let entry_path = entry.path();

        if does_path_match_prefix(&path, &entry_path) {
            if let Some(mut result) = entry_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|str| str.to_owned())
            {
                if entry_path.is_dir() {
                    result.push('/');
                }

                command_palette.result_list.results.push(result);
            }
        }
    }
}

fn on_backspace_file(
    CommandPaletteEventArgs {
        command_palette,
        line_pool,
        time,
        ..
    }: CommandPaletteEventArgs,
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

fn get_command_palette_dir<'a>(
    command_palette: &CommandPalette,
    path: &'a mut PathBuf,
) -> &'a Path {
    let path_separators: &[char] = if cfg!(target_os = "windows") {
        &['/', '\\']
    } else {
        &['/']
    };

    let string = command_palette.doc.to_string();

    let can_path_be_dir = 'dir_check: {
        for separator in path_separators {
            if string.ends_with([*separator]) {
                break 'dir_check true;
            }
        }

        string.is_empty()
    };

    path.clear();
    path.push(".");
    path.push(&string);

    if can_path_be_dir && path.is_dir() {
        path.as_path()
    } else {
        path.parent().unwrap_or(Path::new("."))
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

        if prefix_component.len() > path_component.len() {
            return false;
        }

        for (prefix_char, path_char) in prefix_component.chars().zip(path_component.chars()) {
            if prefix_char.to_ascii_lowercase() != path_char.to_ascii_lowercase() {
                return false;
            }
        }
    }

    true
}
