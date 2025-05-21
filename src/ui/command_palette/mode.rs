use crate::{
    config::theme::Theme,
    ctx::Ctx,
    input::action::Action,
    text::compare::{compare_ignore_ascii_case, score_fuzzy_match},
    ui::{color::Color, editor::Editor, result_list::ResultListSubmitKind},
};

use super::{CommandPalette, CommandPaletteAction, CommandPaletteResult};

pub struct CommandPaletteEventArgs<'a, 'b> {
    pub editor: &'a mut Editor,
    pub ctx: &'a mut Ctx<'b>,
}

impl<'a, 'b> CommandPaletteEventArgs<'a, 'b> {
    pub fn new(editor: &'a mut Editor, ctx: &'a mut Ctx<'b>) -> Self {
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

    fn on_update_results(
        &mut self,
        command_palette: &mut CommandPalette,
        _: CommandPaletteEventArgs,
    ) {
        command_palette.result_list.set_focused_index(0);

        if command_palette.input().is_empty() {
            command_palette
                .result_list
                .results
                .sort_by(|a, b| compare_ignore_ascii_case(&a.text, &b.text));

            return;
        }

        command_palette.result_list.sort_by(|a, b| {
            let input = command_palette.doc.get_line(0).unwrap_or_default();

            let a_score = score_fuzzy_match(&a.text, input);
            let b_score = score_fuzzy_match(&b.text, input);

            b_score.total_cmp(&a_score)
        });
    }
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
