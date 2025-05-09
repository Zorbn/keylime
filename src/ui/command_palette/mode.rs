use crate::{
    config::theme::Theme,
    ctx::Ctx,
    input::action::Action,
    ui::{color::Color, editor::Editor, result_list::ResultListSubmitKind},
};

use super::{CommandPalette, CommandPaletteAction, CommandPaletteResult};

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

    fn on_action(&mut self, _: &mut CommandPalette, _: CommandPaletteEventArgs, _: Action) -> bool {
        false
    }

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

    fn on_display_result<'a>(
        &self,
        result: &'a CommandPaletteResult,
        theme: &Theme,
    ) -> (&'a str, Color) {
        (&result.text, theme.normal)
    }

    fn is_animating(&self) -> bool {
        false
    }
}
