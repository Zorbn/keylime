use std::path::Path;

use crate::{
    pool::{format_pooled, Pooled},
    ui::{editor::Editor, result_list::ResultListSubmitKind},
};

use super::{
    find_in_files_mode::FindInFilesMode,
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction, CommandPaletteMetaData, CommandPaletteResult,
};

pub struct AllDiagnosticsMode;

impl AllDiagnosticsMode {
    fn diagnostic_to_text(
        path: &Path,
        message: &str,
        y: usize,
        editor: &Editor,
    ) -> Option<Pooled<String>> {
        let relative_path = editor
            .current_dir()
            .and_then(|current_dir| path.strip_prefix(current_dir).ok())?;

        let message = message.lines().nth(0).unwrap_or_default();

        Some(format_pooled!(
            "{}:{}: {}",
            relative_path.display(),
            y + 1,
            message,
        ))
    }
}

impl CommandPaletteMode for AllDiagnosticsMode {
    fn title(&self) -> &str {
        "All Diagnostics"
    }

    fn on_open(&mut self, command_palette: &mut CommandPalette, args: CommandPaletteEventArgs) {
        for server in args.ctx.lsp.iter_servers_mut() {
            let encoding = server.position_encoding();

            for (path, diagnostics) in server.all_diagnostics_mut() {
                for diagnostic in diagnostics.encoded() {
                    let Some(text) = Self::diagnostic_to_text(
                        path,
                        &diagnostic.message,
                        diagnostic.range.start.line,
                        args.editor,
                    ) else {
                        continue;
                    };

                    command_palette.result_list.push(CommandPaletteResult {
                        text,
                        meta_data: CommandPaletteMetaData::PathWithEncodedPosition {
                            path: path.clone(),
                            encoding,
                            position: diagnostic.range.start,
                        },
                    });
                }

                for diagnostic in diagnostics.decoded() {
                    let Some(text) = Self::diagnostic_to_text(
                        path,
                        &diagnostic.message,
                        diagnostic.range.start.y,
                        args.editor,
                    ) else {
                        continue;
                    };

                    command_palette.result_list.push(CommandPaletteResult {
                        text,
                        meta_data: CommandPaletteMetaData::PathWithPosition {
                            path: path.clone(),
                            position: diagnostic.range.start,
                        },
                    });
                }
            }
        }
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
        kind: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        FindInFilesMode::jump_to_path_with_position(command_palette, args, kind)
    }
}
