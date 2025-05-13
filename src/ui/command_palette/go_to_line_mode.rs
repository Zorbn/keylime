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
        args: CommandPaletteEventArgs,
        _: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        let (pane, doc_list) = args.editor.focused_pane_and_doc_list_mut();
        let focused_tab_index = pane.focused_tab_index();
        let input = command_palette.input();

        let Ok(line) = input.parse::<usize>() else {
            return CommandPaletteAction::Close;
        };

        let Some((tab, doc)) = pane.get_tab_with_data_mut(focused_tab_index, doc_list) else {
            return CommandPaletteAction::Close;
        };

        doc.jump_cursors(
            Position::new(0, line.saturating_sub(1)),
            false,
            args.ctx.gfx,
        );
        tab.camera.recenter();

        CommandPaletteAction::Close
    }
}
