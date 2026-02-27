use crate::{
    config::Config,
    ctx::Ctx,
    input::action::action_name,
    ui::{
        command_palette::{
            all_actions_mode::AllActionsMode,
            all_diagnostics_mode::AllDiagnosticsMode,
            all_files_mode::AllFilesMode,
            file_explorer_mode::FileExplorerMode,
            find_in_files_mode::FindInFilesMode,
            go_to_line_mode::GoToLineMode,
            search_mode::{SearchAndReplaceMode, SearchMode},
            CommandPalette,
        },
        core::{Ui, WidgetId, WidgetSettings},
        editor::Editor,
        msg::Msg,
        terminal::Terminal,
    },
};

use super::core::WidgetLayout;

pub struct Controller {
    widget_id: WidgetId,
}

impl Controller {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        Self {
            widget_id: ui.new_widget(
                parent_id,
                WidgetSettings {
                    layout: WidgetLayout::Vertical,
                    ..Default::default()
                },
            ),
        }
    }

    pub fn receive_msgs(
        &mut self,
        editor: &mut Editor,
        terminal: &Terminal,
        command_palette: &mut CommandPalette,
        ctx: &mut Ctx,
    ) {
        while let Some(msg) = ctx.ui.msg(self.widget_id) {
            match msg {
                Msg::Action(action_name!(FocusTerminal)) => {
                    let terminal_id = terminal.widget_id();

                    if ctx.ui.is_focused(terminal_id) {
                        ctx.ui.unfocus(terminal_id);
                    } else {
                        ctx.ui.focus(terminal_id);
                    }
                }
                Msg::Action(action_name!(OpenAllActions)) => {
                    command_palette.open(Box::new(AllActionsMode), editor, ctx);
                }
                Msg::Action(action_name!(OpenFileExplorer)) => {
                    command_palette.open(Box::new(FileExplorerMode::new(None)), editor, ctx);
                }
                Msg::Action(action_name!(OpenConfig)) => {
                    let config_dir = Config::dir(ctx.current_dir);

                    command_palette.open(
                        Box::new(FileExplorerMode::new(Some(config_dir))),
                        editor,
                        ctx,
                    );
                }
                Msg::Action(action_name!(OpenSearch)) => {
                    command_palette.open(Box::new(SearchMode::new()), editor, ctx);
                }
                Msg::Action(action_name!(OpenSearchAndReplace)) => {
                    command_palette.open(Box::new(SearchAndReplaceMode::new()), editor, ctx);
                }
                Msg::Action(action_name!(OpenFindInFiles)) => {
                    command_palette.open(Box::new(FindInFilesMode::new()), editor, ctx);
                }
                Msg::Action(action_name!(OpenAllFiles)) => {
                    command_palette.open(Box::new(AllFilesMode::new()), editor, ctx);
                }
                Msg::Action(action_name!(OpenAllDiagnostics)) => {
                    command_palette.open(Box::new(AllDiagnosticsMode), editor, ctx);
                }
                Msg::Action(action_name!(OpenGoToLine)) => {
                    command_palette.open(Box::new(GoToLineMode), editor, ctx);
                }
                _ => ctx.ui.skip(self.widget_id, msg),
            }
        }
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }
}
