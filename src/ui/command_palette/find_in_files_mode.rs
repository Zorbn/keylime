use std::{
    collections::VecDeque,
    fs::{read_dir, DirEntry, ReadDir},
    path::{Path, PathBuf},
    time::Instant,
};

use crate::{
    ctx::Ctx,
    geometry::position::Position,
    pool::{format_pooled, Pooled, PATH_POOL},
    text::{
        cursor_index::CursorIndex,
        doc::{Doc, DocFlags},
    },
    ui::result_list::ResultListSubmitKind,
};

use super::{
    incremental_results::{IncrementalResults, IncrementalStepState},
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction, CommandPaletteMetaData, CommandPaletteResult,
};

const MAX_RESULTS_LEN: usize = 100;

pub struct FindInFilesMode {
    root: PathBuf,
    incremental_results: IncrementalResults,
    pending_doc: Option<Doc>,
    pending_dir_entries: VecDeque<ReadDir>,
}

impl FindInFilesMode {
    pub fn new() -> Self {
        Self {
            root: PathBuf::new(),
            incremental_results: IncrementalResults::new(Some(MAX_RESULTS_LEN)),
            pending_doc: None,
            pending_dir_entries: VecDeque::new(),
        }
    }

    pub fn position_to_result(
        position: Position,
        root: &Path,
        doc: &Doc,
    ) -> Option<CommandPaletteResult> {
        let line = doc.get_line(position.y)?;
        let line_start = doc.line_start(position.y);

        let relative_path = doc
            .path()
            .some()
            .and_then(|path| path.strip_prefix(root).ok())?;

        let text = format_pooled!(
            "{}:{}: {}",
            relative_path.display(),
            position.y + 1,
            &line[line_start..]
        );

        Some(CommandPaletteResult {
            text,
            meta_data: CommandPaletteMetaData::PathWithPosition {
                path: relative_path.into(),
                position,
            },
        })
    }

    pub fn jump_to_path_with_position(
        command_palette: &CommandPalette,
        args: CommandPaletteEventArgs,
        _: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        let Some(CommandPaletteResult {
            meta_data:
                meta_data @ (CommandPaletteMetaData::PathWithPosition { path, .. }
                | CommandPaletteMetaData::PathWithEncodedPosition { path, .. }),
            ..
        }) = command_palette.result_list.get_focused()
        else {
            return CommandPaletteAction::Stay;
        };

        let (pane, doc_list) = args.editor.last_focused_pane_and_doc_list_mut(args.ctx.ui);

        if pane.open_file(path, doc_list, args.ctx).is_err() {
            return CommandPaletteAction::Stay;
        }

        let Some((tab, doc)) = pane.get_focused_tab_with_data_mut(doc_list) else {
            return CommandPaletteAction::Close;
        };

        let position = match meta_data {
            CommandPaletteMetaData::PathWithPosition { position, .. } => *position,
            CommandPaletteMetaData::PathWithEncodedPosition {
                encoding, position, ..
            } => position.decode(*encoding, doc),
            _ => return CommandPaletteAction::Close,
        };

        doc.jump_cursors(position, false, args.ctx.gfx);
        tab.camera.recenter();

        CommandPaletteAction::Close
    }

    fn handle_entry(
        &mut self,
        entry: DirEntry,
        start_time: Instant,
        command_palette: &mut CommandPalette,
        ctx: &mut Ctx,
    ) {
        let path = entry.path();

        if path.is_dir() {
            let is_ignored = path
                .components()
                .next_back()
                .and_then(|dir| dir.as_os_str().to_str())
                .is_some_and(|dir| ctx.config.ignored_dirs.contains(dir));

            if is_ignored {
                return;
            }

            if let Ok(entries) = read_dir(path) {
                self.pending_dir_entries.push_back(entries);
            }

            return;
        }

        let path = Pooled::new(path, &PATH_POOL);
        let mut doc = Doc::new(Some(path), None, DocFlags::RAW);

        if doc.load(ctx).is_ok() {
            self.pending_doc = Some(doc);
            self.handle_doc(start_time, command_palette, ctx);
        }
    }

    fn handle_doc(
        &mut self,
        start_time: Instant,
        command_palette: &mut CommandPalette,
        ctx: &mut Ctx,
    ) -> IncrementalStepState {
        let Some(mut doc) = self.pending_doc.take() else {
            return IncrementalStepState::InProgress;
        };

        while let Some(result_position) = doc.search_forward(
            command_palette.input(),
            doc.cursor(CursorIndex::Main).position,
            false,
            ctx.gfx,
        ) {
            // Ignore additional results on the same line.
            doc.jump_cursor(
                CursorIndex::Main,
                doc.line_end(result_position.y),
                false,
                ctx.gfx,
            );

            let Some(result) = Self::position_to_result(result_position, &self.root, &doc) else {
                continue;
            };

            self.incremental_results.push(result);

            match self
                .incremental_results
                .try_finish(start_time, command_palette)
            {
                IncrementalStepState::InProgress => {}
                IncrementalStepState::DoneWithStep => {
                    self.pending_doc = Some(doc);

                    return IncrementalStepState::DoneWithStep;
                }
                IncrementalStepState::DoneWithAllSteps => {
                    self.clear_pending();

                    return IncrementalStepState::DoneWithAllSteps;
                }
            }
        }

        doc.clear(ctx);

        IncrementalStepState::InProgress
    }

    fn clear_pending(&mut self) {
        self.pending_dir_entries.clear();
        self.pending_doc = None;
    }
}

impl CommandPaletteMode for FindInFilesMode {
    fn title(&self) -> &str {
        "Find in Files"
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
        kind: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        Self::jump_to_path_with_position(command_palette, args, kind)
    }

    fn on_update_results(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
    ) {
        self.clear_pending();
        self.incremental_results.start();

        if command_palette.input().is_empty() {
            self.incremental_results.finish(command_palette);
            return;
        };

        let Some(current_dir) = args.editor.current_dir() else {
            return;
        };

        self.root.clear();
        self.root.push(current_dir);

        if let Ok(entries) = read_dir(&self.root) {
            self.pending_dir_entries.push_back(entries);
        }
    }

    fn on_update(&mut self, command_palette: &mut CommandPalette, args: CommandPaletteEventArgs) {
        if self.incremental_results.is_finished() {
            return;
        }

        let start_time = Instant::now();

        if self.handle_doc(start_time, command_palette, args.ctx)
            != IncrementalStepState::InProgress
        {
            return;
        }

        while let Some(mut entries) = self.pending_dir_entries.pop_front() {
            for entry in entries.by_ref() {
                let Ok(entry) = entry else {
                    continue;
                };

                self.handle_entry(entry, start_time, command_palette, args.ctx);

                match self
                    .incremental_results
                    .try_finish(start_time, command_palette)
                {
                    IncrementalStepState::InProgress => {}
                    IncrementalStepState::DoneWithStep => {
                        self.pending_dir_entries.push_front(entries);
                        return;
                    }
                    IncrementalStepState::DoneWithAllSteps => {
                        self.clear_pending();
                        return;
                    }
                }
            }
        }

        self.incremental_results.finish(command_palette);
    }

    fn is_animating(&self) -> bool {
        !self.incremental_results.is_finished()
    }
}
