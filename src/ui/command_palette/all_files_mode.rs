use std::{
    collections::VecDeque,
    fs::{read_dir, DirEntry, ReadDir},
    path::PathBuf,
    time::Instant,
};

use crate::{
    ctx::Ctx,
    pool::{format_pooled, Pooled, PATH_POOL},
    ui::result_list::ResultListSubmitKind,
};

use super::{
    incremental_results::{IncrementalResults, IncrementalStepState},
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction, CommandPaletteMetaData, CommandPaletteResult,
};

pub struct AllFilesMode {
    root: PathBuf,
    incremental_results: IncrementalResults,
    pending_dir_entries: VecDeque<ReadDir>,
}

impl AllFilesMode {
    pub fn new() -> Self {
        Self {
            root: PathBuf::new(),
            incremental_results: IncrementalResults::new(None),
            pending_dir_entries: VecDeque::new(),
        }
    }

    fn handle_entry(&mut self, entry: DirEntry, ctx: &mut Ctx) {
        let path = Pooled::new(entry.path(), &PATH_POOL);

        if path.is_dir() {
            let is_ignored = path
                .components()
                .last()
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

        let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
            return;
        };

        let Some(parent) = path.parent() else {
            return;
        };

        let text = if let Some(parent) = parent
            .strip_prefix(&self.root)
            .ok()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            format_pooled!("{}: {}", file_name, parent.display())
        } else {
            file_name.into()
        };

        self.incremental_results.push(CommandPaletteResult {
            text,
            meta_data: CommandPaletteMetaData::Path(path),
        });
    }
}

impl CommandPaletteMode for AllFilesMode {
    fn title(&self) -> &str {
        "All Files"
    }

    fn on_open(&mut self, _: &mut CommandPalette, args: CommandPaletteEventArgs) {
        self.pending_dir_entries.clear();
        self.incremental_results.start();

        let Some(current_dir) = args.editor.current_dir() else {
            return;
        };

        self.root.clear();
        self.root.push(current_dir);

        if let Ok(entries) = read_dir(&self.root) {
            self.pending_dir_entries.push_back(entries);
        }
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
        _: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        let Some(CommandPaletteResult {
            meta_data: CommandPaletteMetaData::Path(path),
            ..
        }) = command_palette.result_list.get_focused()
        else {
            return CommandPaletteAction::Stay;
        };

        let (pane, doc_list) = args.editor.last_focused_pane_and_doc_list_mut(args.ctx.ui);

        if pane.open_file(path, doc_list, args.ctx).is_ok() {
            CommandPaletteAction::Close
        } else {
            CommandPaletteAction::Stay
        }
    }

    fn on_update(&mut self, command_palette: &mut CommandPalette, args: CommandPaletteEventArgs) {
        if self.incremental_results.is_finished() {
            return;
        }

        let start_time = Instant::now();

        while let Some(mut entries) = self.pending_dir_entries.pop_front() {
            for entry in entries.by_ref() {
                let Ok(entry) = entry else {
                    continue;
                };

                self.handle_entry(entry, args.ctx);

                match self
                    .incremental_results
                    .try_finish(start_time, command_palette)
                {
                    IncrementalStepState::InProgress => {}
                    IncrementalStepState::DoneWithStep => {
                        self.pending_dir_entries.push_front(entries);
                        self.on_update_results(command_palette, args);
                        return;
                    }
                    IncrementalStepState::DoneWithAllSteps => {
                        self.pending_dir_entries.clear();
                        self.on_update_results(command_palette, args);
                        return;
                    }
                }
            }
        }

        self.incremental_results.finish(command_palette);
        self.on_update_results(command_palette, args);
    }

    fn is_animating(&self) -> bool {
        !self.incremental_results.is_finished()
    }
}
