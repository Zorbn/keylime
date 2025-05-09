use std::{
    fs::{copy, create_dir_all, read_dir, rename},
    io,
    path::{Component, Path, PathBuf},
};

use crate::{
    ctx::Ctx,
    geometry::position::Position,
    input::action::{action_name, Action},
    platform::{gfx::Gfx, recycle::recycle},
    text::{cursor_index::CursorIndex, doc::Doc},
    ui::result_list::ResultListSubmitKind,
};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction, CommandPaletteMetaData, CommandPaletteResult,
};

#[cfg(target_os = "windows")]
const PATH_SEPARATORS: &[&str] = &["/", "\\"];

#[cfg(target_os = "macos")]
const PATH_SEPARATORS: &[&str] = &["/"];

const PREFERRED_PATH_SEPARATOR: &str = "/";

#[derive(Debug, PartialEq, Eq)]
enum FileClipboardState {
    Empty,
    Copy,
    Cut,
}

pub struct FindFileMode {
    clipboard_path: PathBuf,
    clipboard_state: FileClipboardState,
}

impl FindFileMode {
    pub fn new() -> Self {
        Self {
            clipboard_path: PathBuf::new(),
            clipboard_state: FileClipboardState::Empty,
        }
    }
}

impl CommandPaletteMode for FindFileMode {
    fn title(&self) -> &str {
        "Find File"
    }

    fn on_open(&mut self, command_palette: &mut CommandPalette, args: CommandPaletteEventArgs) {
        let (pane, doc_list) = args.editor.get_focused_pane_and_doc_list();
        let focused_tab_index = pane.focused_tab_index();

        let Some((_, doc)) = pane.get_tab_with_data(focused_tab_index, doc_list) else {
            return;
        };

        let Some(current_dir) = args.editor.current_dir() else {
            return;
        };

        let Some(path) = doc
            .path()
            .some()
            .and_then(|path| path.parent())
            .map(|path| path.strip_prefix(current_dir).unwrap_or(path))
        else {
            return;
        };

        let command_doc = &mut command_palette.doc;

        for component in path.components() {
            let Some(string) = component.as_os_str().to_str() else {
                continue;
            };

            command_doc.insert(command_doc.end(), string, args.ctx);

            if !ends_with_path_separator(string) {
                command_doc.insert(command_doc.end(), PREFERRED_PATH_SEPARATOR, args.ctx);
            }
        }
    }

    fn on_action(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
        action: Action,
    ) -> bool {
        let command_doc = &command_palette.doc;
        let cursor = command_doc.get_cursor(CursorIndex::Main);

        match action {
            action_name!(DeleteBackward) => {
                let end = cursor.position;
                let mut start = command_palette.doc.move_position(end, -1, 0, args.ctx.gfx);

                if !is_grapheme_path_separator(command_doc.get_grapheme(start)) {
                    return false;
                }

                start = find_path_component_start(&command_palette.doc, start, args.ctx.gfx);
                command_palette.doc.delete(start, end, args.ctx);

                true
            }
            action_name!(DeleteForward) if cursor.position == command_doc.end() => {
                if let Some(CommandPaletteResult {
                    meta_data: CommandPaletteMetaData::Path(path),
                    ..
                }) = command_palette.result_list.get_selected_result()
                {
                    if path.exists() && recycle(path).is_ok() {
                        self.on_update_results(command_palette, args);
                    }
                }

                true
            }
            action_name!(Copy) | action_name!(Cut) if cursor.get_selection().is_none() => {
                self.clipboard_path.clear();

                if let Some(CommandPaletteResult {
                    meta_data: CommandPaletteMetaData::Path(path),
                    ..
                }) = command_palette.result_list.get_selected_result()
                {
                    self.clipboard_path.push(path);

                    self.clipboard_state = match action {
                        action_name!(Copy) => FileClipboardState::Copy,
                        action_name!(Cut) => FileClipboardState::Cut,
                        _ => unreachable!(),
                    };
                }

                true
            }
            action_name!(Paste) => match self.clipboard_state {
                FileClipboardState::Empty => false,
                FileClipboardState::Copy | FileClipboardState::Cut => {
                    let path = &mut PathBuf::new();
                    get_command_palette_dir(command_palette, path);

                    let Some(file_name) = self.clipboard_path.file_name() else {
                        return true;
                    };

                    path.push(file_name);

                    let is_ok = if self.clipboard_state == FileClipboardState::Copy {
                        copy(&self.clipboard_path, path).is_ok()
                    } else {
                        rename(&self.clipboard_path, path).is_ok()
                    };

                    if is_ok {
                        self.on_update_results(command_palette, args);
                    }

                    true
                }
            },
            _ => false,
        }
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
        _: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        let path = get_command_palette_path(command_palette);

        let input = command_palette.get_input();
        let is_dir = ends_with_path_separator(input);

        if !path.exists() {
            if is_dir {
                let _ = create_dir_all(path);
            } else if let Some(parent) = path.parent() {
                let _ = create_dir_all(parent);
            }
        }

        if is_dir {
            return CommandPaletteAction::Stay;
        }

        let (pane, doc_list) = args.editor.get_focused_pane_and_doc_list_mut();

        if pane
            .open_file(path, doc_list, args.ctx)
            .or_else(|err| {
                if err.kind() == io::ErrorKind::NotFound {
                    pane.new_file(Some(path), doc_list, args.ctx)
                } else {
                    Err(err)
                }
            })
            .is_ok()
        {
            CommandPaletteAction::Close
        } else {
            CommandPaletteAction::Stay
        }
    }

    fn on_complete_result(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
    ) {
        let Some(result) = command_palette.result_list.get_selected_result() else {
            return;
        };

        delete_last_path_component(false, &mut command_palette.doc, args.ctx);

        let line_len = command_palette.doc.get_line_len(0);
        let start = Position::new(line_len, 0);
        command_palette.doc.insert(start, &result.text, args.ctx);
    }

    fn on_update_results(
        &mut self,
        command_palette: &mut CommandPalette,
        _: CommandPaletteEventArgs,
    ) {
        command_palette.result_list.drain();

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
                if let Some(mut result_text) = entry_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|str| str.to_owned())
                {
                    if entry_path.is_dir() {
                        result_text.push_str(PREFERRED_PATH_SEPARATOR);
                    }

                    command_palette
                        .result_list
                        .results
                        .push(CommandPaletteResult {
                            text: result_text,
                            meta_data: CommandPaletteMetaData::Path(entry_path),
                        });
                }
            }
        }
    }
}

fn get_command_palette_path(command_palette: &CommandPalette) -> &Path {
    let input = command_palette.get_input();
    // Trim trailing whitespace, this allows entering "/path/to/file " to create "file"
    // when just "/path/to/file" could auto-complete to another result like "/path/to/filewithlongername"
    let input = input.trim_end();

    Path::new(input)
}

fn get_command_palette_dir<'a>(
    command_palette: &CommandPalette,
    path: &'a mut PathBuf,
) -> &'a Path {
    let input = command_palette.get_input();

    path.clear();
    path.push(".");
    path.push(input);

    let ends_with_dir = matches!(
        path.components().last(),
        Some(Component::CurDir | Component::ParentDir)
    );

    let can_path_be_dir = ends_with_dir || input.is_empty() || ends_with_path_separator(input);

    if can_path_be_dir && path.is_dir() {
        path.as_path()
    } else {
        path.parent().unwrap_or(Path::new("."))
    }
}

fn delete_last_path_component(can_delete_dirs: bool, doc: &mut Doc, ctx: &mut Ctx) {
    let end = doc.get_line_end(0);

    let find_start = if can_delete_dirs {
        doc.move_position(end, -1, 0, ctx.gfx)
    } else {
        end
    };

    let start = find_path_component_start(doc, find_start, ctx.gfx);

    doc.delete(start, end, ctx);
}

fn is_grapheme_path_separator(grapheme: &str) -> bool {
    PATH_SEPARATORS
        .iter()
        .any(|separator| *separator == grapheme)
}

fn ends_with_path_separator(text: &str) -> bool {
    PATH_SEPARATORS
        .iter()
        .any(|separator| text.ends_with(separator))
}

fn find_path_component_start(doc: &Doc, position: Position, gfx: &mut Gfx) -> Position {
    let mut start = position;

    while start > Position::ZERO {
        let next_start = doc.move_position(start, -1, 0, gfx);

        if is_grapheme_path_separator(doc.get_grapheme(next_start)) {
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

        if !prefix_component.eq_ignore_ascii_case(&path_component[..prefix_component.len()]) {
            return false;
        }
    }

    true
}
