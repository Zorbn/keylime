use crate::ui::result_list::ResultListSubmitKind;

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction,
};

pub struct RenameMode;

impl CommandPaletteMode for RenameMode {
    fn title(&self) -> &str {
        "Rename"
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        CommandPaletteEventArgs {
            pane,
            doc_list,
            ctx,
            ..
        }: CommandPaletteEventArgs,
        kind: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        if kind != ResultListSubmitKind::Normal {
            return CommandPaletteAction::Stay;
        }

        let focused_tab_index = pane.focused_tab_index();
        let input = command_palette.get_input();

        let Some((_, doc)) = pane.get_tab_with_data(focused_tab_index, doc_list) else {
            return CommandPaletteAction::Close;
        };

        doc.lsp_rename(input, ctx);

        CommandPaletteAction::Close
    }
}
