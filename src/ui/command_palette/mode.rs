use crate::{
    ctx::Ctx,
    ui::{editor::Editor, result_list::ResultListSubmitKind},
};

use super::{CommandPalette, CommandPaletteAction};

pub struct CommandPaletteEventArgs<'a, 'b> {
    pub editor: &'a mut Editor,
    pub ctx: &'a mut Ctx<'b>,
}

impl<'a, 'b> CommandPaletteEventArgs<'a, 'b> {
    pub fn new(editor: &'a mut Editor, ctx: &'a mut Ctx<'b>) -> CommandPaletteEventArgs<'a, 'b> {
        CommandPaletteEventArgs { editor, ctx }
    }
}

pub trait CommandPaletteMode {
    fn title(&self) -> &str {
        "Unnamed"
    }

    fn on_open(&mut self, _: &mut CommandPalette, _: CommandPaletteEventArgs) {}

    fn on_submit(
        &mut self,
        _: &mut CommandPalette,
        _: CommandPaletteEventArgs,
        _: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        CommandPaletteAction::Stay
    }

    fn on_complete_result(&mut self, _: &mut CommandPalette, _: CommandPaletteEventArgs) {}

    fn on_update_results(&mut self, _: &mut CommandPalette, _: CommandPaletteEventArgs) {}

    fn on_update(&mut self, _: &mut CommandPalette, _: CommandPaletteEventArgs) {}

    fn on_backspace(&mut self, _: &mut CommandPalette, _: CommandPaletteEventArgs) -> bool {
        false
    }

    fn is_animating(&self) -> bool {
        false
    }
}
