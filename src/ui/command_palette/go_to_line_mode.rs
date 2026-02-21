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
        let input = command_palette.input();

        let Ok(line) = input.parse::<usize>() else {
            return CommandPaletteAction::Close;
        };

        let (pane, doc_list) = args.editor.last_focused_pane_and_doc_list_mut(args.ctx.ui);

        let Some((tab, doc)) = pane.get_focused_tab_with_data_mut(doc_list, args.ctx.ui) else {
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
