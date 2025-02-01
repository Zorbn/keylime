use std::{
    env::current_dir,
    fs::{create_dir_all, read_dir, File},
    path::{Component, Path, PathBuf},
};

use crate::{
    geometry::position::Position,
    platform::recycle::recycle,
    text::{cursor_index::CursorIndex, doc::Doc, line_pool::LinePool},
    ui::result_list::ResultListSubmitKind,
};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction,
};

const PREFERRED_PATH_SEPARATOR: char = '/';

pub const MODE_OPEN_FILE: &CommandPaletteMode = &CommandPaletteMode {
    title: "Find File",
    on_open,
    on_submit,
    on_complete_result,
    on_update_results,
    on_backspace,
    ..CommandPaletteMode::default()
};

fn on_open(
    command_palette: &mut CommandPalette,
    CommandPaletteEventArgs {
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

        if !str.ends_with(is_char_path_separator) {
            let position = command_doc.end();
            command_doc.insert(position, [PREFERRED_PATH_SEPARATOR], line_pool, time);
        }
    }
}

fn on_submit(
    command_palette: &mut CommandPalette,
    mut args: CommandPaletteEventArgs,
    kind: ResultListSubmitKind,
) -> CommandPaletteAction {
    if !matches!(
        kind,
        ResultListSubmitKind::Normal | ResultListSubmitKind::Delete
    ) {
        return CommandPaletteAction::Stay;
    }

    let CommandPaletteEventArgs {
        pane,
        doc_list,
        config,
        line_pool,
        time,
    } = &mut args;

    let string = command_palette.doc.to_string();
    // Trim trailing whitespace, this allows entering "/path/to/file " to create "file"
    // when just "/path/to/file" could auto-complete to another result like "/path/to/filewithlongername"
    let string = string.trim_end();

    let path = Path::new(string);

    if kind == ResultListSubmitKind::Delete {
        if path.exists() && recycle(path).is_ok() {
            delete_last_path_component(true, &mut command_palette.doc, line_pool, *time);
        }

        return CommandPaletteAction::Stay;
    }

    if !path.exists() {
        if string.ends_with(is_char_path_separator) {
            let _ = create_dir_all(path);

            on_update_results(command_palette, args);

            return CommandPaletteAction::Stay;
        } else {
            if let Some(parent) = path.parent() {
                let _ = create_dir_all(parent);
            }

            let _ = File::create(path);
        }
    }

    if pane
        .open_file(path, doc_list, config, line_pool, *time)
        .is_ok()
    {
        CommandPaletteAction::Close
    } else {
        CommandPaletteAction::Stay
    }
}

fn on_complete_result(
    command_palette: &mut CommandPalette,
    CommandPaletteEventArgs {
        line_pool, time, ..
    }: CommandPaletteEventArgs,
) {
    let Some(result) = command_palette.result_list.get_selected_result() else {
        return;
    };

    delete_last_path_component(false, &mut command_palette.doc, line_pool, time);

    let line_len = command_palette.doc.get_line_len(0);
    let mut start = Position::new(line_len, 0);

    for c in result.chars() {
        command_palette.doc.insert(start, [c], line_pool, time);
        start = command_palette
            .doc
            .move_position(start, Position::new(1, 0));
    }
}

fn on_update_results(command_palette: &mut CommandPalette, _: CommandPaletteEventArgs) {
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
                    result.push(PREFERRED_PATH_SEPARATOR);
                }

                command_palette.result_list.results.push(result);
            }
        }
    }
}

fn on_backspace(
    command_palette: &mut CommandPalette,
    CommandPaletteEventArgs {
        line_pool, time, ..
    }: CommandPaletteEventArgs,
) -> bool {
    let cursor = command_palette.doc.get_cursor(CursorIndex::Main);
    let end = cursor.position;
    let mut start = command_palette.doc.move_position(end, Position::new(-1, 0));

    if is_char_path_separator(command_palette.doc.get_char(start)) {
        start = find_path_component_start(&command_palette.doc, start);

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
    let string = command_palette.doc.to_string();

    path.clear();
    path.push(".");
    path.push(&string);

    let ends_with_dir = matches!(
        path.components().last(),
        Some(Component::CurDir | Component::ParentDir)
    );

    let can_path_be_dir =
        ends_with_dir || string.is_empty() || string.ends_with(is_char_path_separator);

    if can_path_be_dir && path.is_dir() {
        path.as_path()
    } else {
        path.parent().unwrap_or(Path::new("."))
    }
}

fn delete_last_path_component(
    can_delete_dirs: bool,
    doc: &mut Doc,
    line_pool: &mut LinePool,
    time: f32,
) {
    let line_len = doc.get_line_len(0);
    let end = Position::new(line_len, 0);

    let find_start = if can_delete_dirs {
        doc.move_position(end, Position::new(-1, 0))
    } else {
        end
    };

    let start = find_path_component_start(doc, find_start);

    doc.delete(start, end, line_pool, time);
}

fn is_char_path_separator(c: char) -> bool {
    if cfg!(target_os = "windows") {
        matches!(c, '/' | '\\')
    } else {
        matches!(c, '/')
    }
}

fn find_path_component_start(doc: &Doc, position: Position) -> Position {
    let mut start = position;

    while start > Position::zero() {
        let next_start = doc.move_position(start, Position::new(-1, 0));

        if is_char_path_separator(doc.get_char(next_start)) {
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
