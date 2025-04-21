use std::{
    cmp::Ordering,
    collections::VecDeque,
    env::current_dir,
    fs::{read_dir, DirEntry, ReadDir},
    path::PathBuf,
    time::Instant,
};

use crate::{ctx::Ctx, ui::result_list::ResultListSubmitKind};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction, CommandPaletteMetaData, CommandPaletteResult,
};

const TARGET_FIND_TIME: f32 = 0.005;

pub struct AllFilesMode {
    root: PathBuf,
    needs_new_results: bool,
    pending_dir_entries: VecDeque<ReadDir>,
    pending_results: Vec<CommandPaletteResult>,
}

impl AllFilesMode {
    pub fn new() -> Self {
        Self {
            root: PathBuf::new(),
            needs_new_results: false,
            pending_dir_entries: VecDeque::new(),
            pending_results: Vec::new(),
        }
    }

    fn handle_entry(&mut self, entry: DirEntry, ctx: &mut Ctx) {
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

        let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
            return;
        };

        let Some(parent) = path.parent() else {
            return;
        };

        let result_text = if let Some(parent) = parent
            .strip_prefix(&self.root)
            .ok()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            format!("{}: {}", file_name, parent.display())
        } else {
            file_name.into()
        };

        self.pending_results.push(CommandPaletteResult {
            text: result_text,
            meta_data: CommandPaletteMetaData::Path(path),
        });
    }
}

impl CommandPaletteMode for AllFilesMode {
    fn title(&self) -> &str {
        "All Files"
    }

    fn on_open(&mut self, _: &mut CommandPalette, _: CommandPaletteEventArgs) {
        self.pending_dir_entries.clear();
        self.needs_new_results = true;

        let Ok(current_dir) = current_dir() else {
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
            meta_data: CommandPaletteMetaData::Path(path),
            ..
        }) = command_palette.result_list.get_selected_result()
        else {
            return CommandPaletteAction::Stay;
        };

        if pane.open_file(path, doc_list, ctx).is_ok() {
            CommandPaletteAction::Close
        } else {
            CommandPaletteAction::Stay
        }
    }

    fn on_update_results(
        &mut self,
        command_palette: &mut CommandPalette,
        _: CommandPaletteEventArgs,
    ) {
        command_palette.result_list.reset_selected_result();

        if command_palette.get_input().is_empty() {
            command_palette
                .result_list
                .results
                .sort_by(|a, b| compare_ignore_ascii_case(&a.text, &b.text));

            return;
        }

        command_palette.result_list.results.sort_by(|a, b| {
            let input = command_palette.doc.get_line(0).unwrap_or_default();

            let a_score = score_fuzzy_match(&a.text, input);
            let b_score = score_fuzzy_match(&b.text, input);

            b_score.partial_cmp(&a_score).unwrap_or(Ordering::Equal)
        });
    }

    fn on_update(&mut self, command_palette: &mut CommandPalette, args: CommandPaletteEventArgs) {
        if !self.needs_new_results {
            return;
        }

        let start_time = Instant::now();

        while let Some(mut entries) = self.pending_dir_entries.pop_front() {
            for entry in entries.by_ref() {
                let Ok(entry) = entry else {
                    continue;
                };

                self.handle_entry(entry, args.ctx);

                if start_time.elapsed().as_secs_f32() > TARGET_FIND_TIME {
                    self.pending_dir_entries.push_front(entries);
                    return;
                }
            }
        }

        command_palette.result_list.drain();
        command_palette
            .result_list
            .results
            .append(&mut self.pending_results);

        self.needs_new_results = false;
        self.on_update_results(command_palette, args);
    }

    fn is_animating(&self) -> bool {
        self.needs_new_results
    }
}

fn compare_ignore_ascii_case(a: &str, b: &str) -> Ordering {
    for (a_char, b_char) in a.chars().zip(b.chars()) {
        let a_char = a_char.to_ascii_lowercase();
        let b_char = b_char.to_ascii_lowercase();

        let ordering = a_char.cmp(&b_char);

        if ordering != Ordering::Equal {
            return ordering;
        }
    }

    Ordering::Equal
}

fn score_fuzzy_match(path: &str, input: &str) -> f32 {
    const AWARD_DISTANCE_FALLOFF: f32 = 0.8;
    const AWARD_MATCH_BONUS: f32 = 1.0;
    const AWARD_MAX_AFTER_MISMATCH: f32 = 1.0;

    let mut score = 0.0;
    let mut next_match_award = 1.0;

    let mut path_chars = path.chars();
    let mut input_chars = input.chars().peekable();

    while let Some((path_char, input_char)) = path_chars.next().zip(input_chars.peek()) {
        let path_char = path_char.to_ascii_lowercase();
        let input_char = input_char.to_ascii_lowercase();

        if path_char == input_char {
            score += next_match_award;
            next_match_award += AWARD_MATCH_BONUS;

            input_chars.next();
        } else if score > 0.0 {
            next_match_award =
                AWARD_MAX_AFTER_MISMATCH.min(next_match_award * AWARD_DISTANCE_FALLOFF);
        }
    }

    score
}
