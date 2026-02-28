use std::path::Path;

use crate::{
    config::theme::Theme,
    lsp::types::DecodedDiagnostic,
    pool::{format_pooled, Pooled},
    ui::{color::Color, result_list::ResultListSubmitKind},
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
        current_dir: &Path,
    ) -> Option<Pooled<String>> {
        let relative_path = path.strip_prefix(current_dir).ok()?;

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
                        args.ctx.current_dir,
                    ) else {
                        continue;
                    };

                    command_palette.result_list.push(CommandPaletteResult {
                        text,
                        meta_data: CommandPaletteMetaData::DiagnosticWithEncodedPosition {
                            path: path.clone(),
                            encoding,
                            position: diagnostic.range.start,
                            severity: diagnostic.severity,
                        },
                    });
                }

                for diagnostic in diagnostics.decoded() {
                    let Some(text) = Self::diagnostic_to_text(
                        path,
                        &diagnostic.message,
                        diagnostic.range.start.y,
                        args.ctx.current_dir,
                    ) else {
                        continue;
                    };

                    command_palette.result_list.push(CommandPaletteResult {
                        text,
                        meta_data: CommandPaletteMetaData::DiagnosticWithPosition {
                            path: path.clone(),
                            position: diagnostic.range.start,
                            severity: diagnostic.severity,
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

    fn on_display_result<'a>(
        &self,
        result: &'a CommandPaletteResult,
        theme: &Theme,
    ) -> (&'a str, Color) {
        let color = if let CommandPaletteMetaData::DiagnosticWithPosition { severity, .. }
        | CommandPaletteMetaData::DiagnosticWithEncodedPosition {
            severity, ..
        } = &result.meta_data
        {
            DecodedDiagnostic::severity_color(*severity, theme)
        } else {
            theme.normal
        };

        (result.text.as_str(), color)
    }
}
