use std::{
    collections::VecDeque,
    env::current_dir,
    fs::{read_dir, DirEntry, ReadDir},
    path::PathBuf,
    time::Instant,
};

use crate::{
    ctx::Ctx,
    text::{
        cursor_index::CursorIndex,
        doc::{Doc, DocKind},
    },
    ui::result_list::ResultListSubmitKind,
};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction, CommandPaletteMetaData, CommandPaletteResult,
};

const MAX_RESULTS: usize = 100;
const TARGET_FIND_TIME: f32 = 0.005;

pub struct FindInFilesMode {
    root: PathBuf,
    needs_new_results: bool,
    pending_doc: Option<Doc>,
    pending_dir_entries: VecDeque<ReadDir>,
    pending_results: Vec<CommandPaletteResult>,
}

impl FindInFilesMode {
    pub fn new() -> Self {
        Self {
            root: PathBuf::new(),
            needs_new_results: false,
            pending_doc: None,
            pending_dir_entries: VecDeque::new(),
            pending_results: Vec::new(),
        }
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

        let mut doc = Doc::new(Some(path), &mut ctx.buffers.lines, None, DocKind::Output);

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
    ) {
        let Some(mut doc) = self.pending_doc.take() else {
            return;
        };

        while let Some(result_position) = doc.search_forward(
            command_palette.get_input(),
            doc.get_cursor(CursorIndex::Main).position,
            false,
        ) {
            // Ignore additional results on the same line.
            doc.jump_cursor(
                CursorIndex::Main,
                doc.get_line_end(result_position.y),
                false,
                ctx.gfx,
            );

            let Some(line) = doc.get_line(result_position.y) else {
                continue;
            };

            let line_start = doc.get_line_start(result_position.y);

            let Some(relative_path) = doc
                .path()
                .on_drive()
                .and_then(|path| path.strip_prefix(&self.root).ok())
            else {
                continue;
            };

            let result_text = format!(
                "{}:{}: {}",
                relative_path.display(),
                result_position.y + 1,
                &line[line_start..]
            );

            self.pending_results.push(CommandPaletteResult {
                text: result_text,
                meta_data: CommandPaletteMetaData::PathWithPosition {
                    path: relative_path.to_owned(),
                    position: result_position,
                },
            });

            if self.try_finish_finding(start_time, command_palette) {
                self.pending_doc = Some(doc);
                return;
            }
        }

        doc.clear(ctx);
    }

    fn try_finish_finding(
        &mut self,
        start_time: Instant,
        command_palette: &mut CommandPalette,
    ) -> bool {
        if self.pending_results.len() >= MAX_RESULTS {
            self.flush_pending_results(command_palette);
            return true;
        }

        start_time.elapsed().as_secs_f32() > TARGET_FIND_TIME
    }

    fn flush_pending_results(&mut self, command_palette: &mut CommandPalette) {
        if !self.needs_new_results {
            return;
        }

        self.needs_new_results = false;

        command_palette.result_list.drain();
        command_palette
            .result_list
            .results
            .append(&mut self.pending_results);

        self.clear_pending();
    }

    fn clear_pending(&mut self) {
        self.pending_dir_entries.clear();
        self.pending_doc = None;
        self.pending_results.clear();
    }
}

impl CommandPaletteMode for FindInFilesMode {
    fn title(&self) -> &str {
        "Find in Files"
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        CommandPaletteEventArgs {
            pane,
            doc_list,
            ctx,
        }: CommandPaletteEventArgs,
        kind: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        if !matches!(
            kind,
            ResultListSubmitKind::Normal | ResultListSubmitKind::Alternate
        ) {
            return CommandPaletteAction::Stay;
        }

        let Some(CommandPaletteResult {
            meta_data: CommandPaletteMetaData::PathWithPosition { path, position },
            ..
        }) = command_palette.result_list.get_selected_result()
        else {
            return CommandPaletteAction::Stay;
        };

        if pane.open_file(path, doc_list, ctx).is_err() {
            return CommandPaletteAction::Stay;
        }

        let focused_tab_index = pane.focused_tab_index();

        let Some((tab, doc)) = pane.get_tab_with_data_mut(focused_tab_index, doc_list) else {
            return CommandPaletteAction::Close;
        };

        doc.jump_cursors(*position, false, ctx.gfx);
        tab.camera.recenter();

        CommandPaletteAction::Close
    }

    fn on_update_results(
        &mut self,
        command_palette: &mut CommandPalette,
        _: CommandPaletteEventArgs,
    ) {
        self.clear_pending();
        self.needs_new_results = true;

        if command_palette.get_input().is_empty() {
            self.flush_pending_results(command_palette);
            return;
        };

        let Ok(current_dir) = current_dir() else {
            return;
        };

        self.root.clear();
        self.root.push(current_dir);

        if let Ok(entries) = read_dir(&self.root) {
            self.pending_dir_entries.push_back(entries);
        }
    }

    fn on_update(
        &mut self,
        command_palette: &mut CommandPalette,
        CommandPaletteEventArgs { ctx, .. }: CommandPaletteEventArgs,
    ) {
        if !self.needs_new_results {
            return;
        }

        let start_time = Instant::now();

        self.handle_doc(start_time, command_palette, ctx);

        if self.try_finish_finding(start_time, command_palette) {
            return;
        }

        while let Some(mut entries) = self.pending_dir_entries.pop_front() {
            for entry in entries.by_ref() {
                let Ok(entry) = entry else {
                    continue;
                };

                self.handle_entry(entry, start_time, command_palette, ctx);

                if self.try_finish_finding(start_time, command_palette) {
                    self.pending_dir_entries.push_front(entries);
                    return;
                }
            }
        }

        self.flush_pending_results(command_palette);
    }

    fn is_animating(&self) -> bool {
        self.needs_new_results
    }
}
