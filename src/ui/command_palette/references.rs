use crate::ui::result_list::ResultListSubmitKind;

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction, CommandPaletteMetaData, CommandPaletteResult,
};

pub struct References {
    results: Vec<CommandPaletteResult>,
}

impl References {
    pub fn new(results: Vec<CommandPaletteResult>) -> Self {
        Self { results }
    }
}

impl CommandPaletteMode for References {
    fn title(&self) -> &str {
        "References"
    }

    fn on_open(&mut self, command_palette: &mut CommandPalette, _: CommandPaletteEventArgs) {
        command_palette
            .result_list
            .results
            .append(&mut self.results);
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
        // TODO: This is the same logic as on_submit for find in files mode.
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
}
