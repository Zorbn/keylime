use std::{
    env::current_dir,
    fs::read_dir,
    path::{Path, PathBuf},
    time::Instant,
};

use crate::{
    ctx::Ctx,
    geometry::position::Position,
    text::doc::{Doc, DocKind},
    ui::result_list::ResultListSubmitKind,
};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction,
};

const MAX_RESULTS: usize = 100;
const MAX_FIND_TIME: f32 = 0.1;

pub struct FindInFilesMode;

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

        let Some(selected_result) = command_palette.result_list.get_selected_result() else {
            return CommandPaletteAction::Stay;
        };

        let mut result_parts = selected_result.split(':');

        let Some(path) = result_parts.next().map(Path::new) else {
            return CommandPaletteAction::Stay;
        };

        let Some(line) = result_parts
            .next()
            .and_then(|line| line.parse::<usize>().ok())
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

        doc.jump_cursors(Position::new(0, line.saturating_sub(1)), false, ctx.gfx);
        tab.camera.recenter();

        CommandPaletteAction::Close
    }

    fn on_update_results(
        &mut self,
        command_palette: &mut CommandPalette,
        CommandPaletteEventArgs { ctx, .. }: CommandPaletteEventArgs,
    ) {
        let Ok(current_dir) = current_dir() else {
            return;
        };

        let search_term = command_palette.doc.get_line(0).unwrap_or_default();

        if search_term.is_empty() {
            return;
        };

        let results = &mut command_palette.result_list.results;

        let start = Instant::now();

        handle_dir(&current_dir, &current_dir, search_term, start, results, ctx);
    }
}

fn handle_dir(
    root: &Path,
    path: &Path,
    search_term: &str,
    start: Instant,
    results: &mut Vec<String>,
    ctx: &mut Ctx,
) {
    if path
        .components()
        .last()
        .and_then(|dir| dir.as_os_str().to_str())
        .is_some_and(|dir| ctx.config.ignored_dirs.contains(dir))
    {
        return;
    }

    let Ok(entries) = read_dir(path) else {
        return;
    };

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };

        let path = entry.path();

        if path.is_dir() {
            handle_dir(root, &path, search_term, start, results, ctx);
        } else {
            handle_file(root, path, search_term, start, results, ctx);
        }

        if should_stop_finding(start, results) {
            break;
        }
    }
}

fn handle_file(
    root: &Path,
    path: PathBuf,
    search_term: &str,
    start: Instant,
    results: &mut Vec<String>,
    ctx: &mut Ctx,
) {
    let mut doc = Doc::new(Some(path), &mut ctx.buffers.lines, None, DocKind::MultiLine);

    if doc.load(ctx).is_err() {
        doc.clear(&mut ctx.buffers.lines);
        return;
    }

    let mut search_start = Position::ZERO;

    while let Some(result_position) = doc.search_forward(search_term, search_start, false) {
        search_start = result_position;
        // Ignore additional results on the same line.
        search_start.y += 1;

        let Some(line) = doc.get_line(result_position.y) else {
            continue;
        };

        let line_start = doc.get_line_start(result_position.y);

        let Some(relative_path) = doc
            .path()
            .on_drive()
            .and_then(|path| path.strip_prefix(root).ok())
        else {
            continue;
        };

        let result = format!(
            "{}:{}: {}",
            relative_path.display(),
            result_position.y + 1,
            &line[line_start..]
        );

        results.push(result);

        if should_stop_finding(start, results) {
            results.push("...".into());
            break;
        }
    }

    doc.clear(&mut ctx.buffers.lines);
}

fn should_stop_finding(start: Instant, results: &[String]) -> bool {
    results.len() >= MAX_RESULTS || start.elapsed().as_secs_f32() > MAX_FIND_TIME
}
