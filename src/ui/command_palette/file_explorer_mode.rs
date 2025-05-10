use std::{
    fs::{copy, create_dir_all, read_dir, rename},
    io,
    path::{Component, Path, PathBuf},
};

use crate::{
    config::theme::Theme,
    ctx::Ctx,
    geometry::position::Position,
    input::action::{action_name, Action},
    platform::{gfx::Gfx, recycle::recycle},
    text::{cursor_index::CursorIndex, doc::Doc},
    ui::{color::Color, result_list::ResultListSubmitKind},
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

pub struct FileExplorerMode {
    clipboard_path: PathBuf,
    clipboard_state: FileClipboardState,

    renaming_result_index: Option<usize>,
    input_backup: String,
}

impl FileExplorerMode {
    pub fn new() -> Self {
        Self {
            clipboard_path: PathBuf::new(),
            clipboard_state: FileClipboardState::Empty,

            renaming_result_index: None,
            input_backup: String::new(),
        }
    }

    fn clear_clipboard(&mut self) {
        self.clipboard_path.clear();
        self.clipboard_state = FileClipboardState::Empty;
    }

    fn begin_renaming(&mut self, command_palette: &mut CommandPalette, ctx: &mut Ctx) {
        let focused_result_index = command_palette.result_list.results.focused_index();

        self.renaming_result_index = Some(focused_result_index);
        self.input_backup.clear();

        let command_doc = &mut command_palette.doc;
        command_doc.collect_string(Position::ZERO, command_doc.end(), &mut self.input_backup);
        command_doc.clear(ctx);

        command_doc.insert(
            Position::ZERO,
            command_palette
                .result_list
                .results
                .get_focused()
                .map(|result| result.text.as_str())
                .unwrap_or_default(),
            ctx,
        );
    }

    fn end_renaming(&mut self, command_palette: &mut CommandPalette, ctx: &mut Ctx) {
        let Some(renaming_index) = self.renaming_result_index else {
            return;
        };

        if let Some(CommandPaletteResult {
            text,
            meta_data: CommandPaletteMetaData::Path(path),
        }) = command_palette.result_list.results.get(renaming_index)
        {
            let mut new_path = path.clone();
            new_path.set_file_name(text);

            let _ = rename(path, new_path);
        }

        command_palette.doc.clear(ctx);
        command_palette
            .doc
            .insert(Position::ZERO, &self.input_backup, ctx);

        self.renaming_result_index = None;

        let focused_result_index = command_palette.result_list.results.focused_index();
        self.update_results(command_palette, Some(focused_result_index), None);
    }

    fn update_results(
        &self,
        command_palette: &mut CommandPalette,
        focused_result_index: Option<usize>,
        deleted_path: Option<PathBuf>,
    ) {
        command_palette.result_list.drain();

        let input = if self.renaming_result_index.is_some() {
            &self.input_backup
        } else {
            command_palette.get_input()
        };

        let mut path = PathBuf::new();
        let dir = get_input_dir(input, &mut path);

        let Ok(entries) = read_dir(dir) else {
            return;
        };

        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };

            let entry_path = entry.path();

            if Some(&entry_path) == deleted_path.as_ref() {
                continue;
            }

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

        if let Some(renaming_result) = self
            .renaming_result_index
            .and_then(|index| command_palette.result_list.results.get_mut(index))
        {
            renaming_result.text = command_palette
                .doc
                .get_line(0)
                .unwrap_or_default()
                .to_owned();
        }

        if let Some(focused_result_index) = focused_result_index.or(self.renaming_result_index) {
            command_palette
                .result_list
                .results
                .set_focused_index(focused_result_index);
        }
    }
}

impl CommandPaletteMode for FileExplorerMode {
    fn title(&self) -> &str {
        if self.renaming_result_index.is_some() {
            "File Explorer: Renaming"
        } else {
            "File Explorer"
        }
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
        if self.renaming_result_index.is_some() {
            return false;
        }

        let command_doc = &mut command_palette.doc;
        let cursor = command_doc.get_cursor(CursorIndex::Main);

        if command_doc.cursors_len() != 1 {
            return false;
        }

        match action {
            action_name!(DeleteBackward) => {
                let end = cursor.position;
                let mut start = command_doc.move_position(end, -1, 0, args.ctx.gfx);

                if !is_grapheme_path_separator(command_doc.get_grapheme(start)) {
                    return false;
                }

                start = find_path_component_start(command_doc, start, args.ctx.gfx);
                command_doc.delete(start, end, args.ctx);

                true
            }
            action_name!(DeleteForward) if cursor.position == command_doc.end() => {
                let focused_result_index = command_palette.result_list.results.focused_index();
                let mut deleted_path = None;

                if let Some(CommandPaletteResult {
                    meta_data: CommandPaletteMetaData::Path(path),
                    ..
                }) = command_palette.result_list.results.remove()
                {
                    if path.exists() && recycle(&path).is_ok() {
                        deleted_path = Some(path);
                    }
                }

                self.update_results(command_palette, Some(focused_result_index), deleted_path);

                true
            }
            action_name!(Copy) | action_name!(Cut) => {
                self.clear_clipboard();

                if cursor.get_selection().is_some() {
                    return false;
                }

                if let Some(CommandPaletteResult {
                    meta_data: CommandPaletteMetaData::Path(path),
                    ..
                }) = command_palette.result_list.results.get_focused()
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
                    let focused_result_index = command_palette.result_list.results.focused_index();

                    let input = command_palette.get_input();
                    let mut path = PathBuf::new();
                    get_input_dir(input, &mut path);

                    let Some(file_name) = self.clipboard_path.file_name() else {
                        return true;
                    };

                    path.push(file_name);

                    let is_ok = if self.clipboard_state == FileClipboardState::Copy {
                        update_path_for_copy(&mut path, args.ctx.buffers.text.get_mut());

                        copy(&self.clipboard_path, path).is_ok()
                    } else {
                        rename(&self.clipboard_path, path)
                            .inspect(|_| self.clear_clipboard())
                            .is_ok()
                    };

                    if is_ok {
                        self.update_results(command_palette, Some(focused_result_index), None);
                    }

                    true
                }
            },
            action_name!(Rename) => {
                self.begin_renaming(command_palette, args.ctx);

                true
            }
            _ => false,
        }
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
        _: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        if self.renaming_result_index.is_some() {
            self.end_renaming(command_palette, args.ctx);

            return CommandPaletteAction::Stay;
        }

        let input = command_palette.get_input();
        let path = get_input_path(input);
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
        if self.renaming_result_index.is_some() {
            return;
        }

        let Some(result) = command_palette.result_list.results.get_focused() else {
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
        self.update_results(command_palette, None, None);
    }

    fn on_display_result<'a>(
        &self,
        result: &'a CommandPaletteResult,
        theme: &Theme,
    ) -> (&'a str, Color) {
        let default_display = (result.text.as_str(), theme.normal);

        if self.clipboard_state != FileClipboardState::Cut {
            return default_display;
        }

        if let CommandPaletteResult {
            meta_data: CommandPaletteMetaData::Path(path),
            ..
        } = result
        {
            if path == &self.clipboard_path {
                return (&result.text, theme.subtle);
            }
        }

        default_display
    }
}

fn get_input_path(input: &str) -> &Path {
    // Trim trailing whitespace, this allows entering "/path/to/file " to create "file"
    // when just "/path/to/file" could auto-complete to another result like "/path/to/filewithlongername"
    let input = input.trim_end();

    Path::new(input)
}

fn get_input_dir<'a>(input: &str, path: &'a mut PathBuf) -> &'a Path {
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

fn update_path_for_copy(path: &mut PathBuf, buffer: &mut String) {
    if !path.exists() {
        return;
    }

    let Some(file_stem) = path.file_stem().and_then(|file_stem| file_stem.to_str()) else {
        return;
    };

    buffer.push_str(file_stem);
    buffer.push_str(" (copy)");

    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        buffer.push('.');
        buffer.push_str(extension);
    }

    path.set_file_name(buffer);
}
