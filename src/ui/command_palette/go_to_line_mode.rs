use crate::{geometry::position::Position, ui::result_list::ResultListSubmitKind};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction,
};

pub struct GoToLineMode;

impl CommandPaletteMode for GoToLineMode {
    fn title(&self) -> &str {
        "Go to Line"
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

        let Ok(line) = input.parse::<usize>() else {
            return CommandPaletteAction::Close;
        };

        let Some((tab, doc)) = pane.get_tab_with_data_mut(focused_tab_index, doc_list) else {
            return CommandPaletteAction::Close;
        };

        doc.jump_cursors(Position::new(0, line.saturating_sub(1)), false, ctx.gfx);
        tab.camera.recenter();

        CommandPaletteAction::Close
    }
}
