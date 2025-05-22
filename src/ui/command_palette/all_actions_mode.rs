use crate::{
    input::action::{Action, ActionName},
    pool::format_pooled,
    text::grapheme::{self, CharCursor},
    ui::result_list::ResultListSubmitKind,
};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction, CommandPaletteMetaData, CommandPaletteResult,
};

pub struct AllActionsMode;

impl CommandPaletteMode for AllActionsMode {
    fn title(&self) -> &str {
        "All Actions"
    }

    fn on_open(&mut self, command_palette: &mut CommandPalette, _: CommandPaletteEventArgs) {
        for action_name in ActionName::VARIANTS {
            let mut text = format_pooled!("{:?}", action_name);

            let mut char_cursor = CharCursor::new(text.len(), text.len());
            let mut has_uppercase = false;

            while char_cursor.index() > 0 {
                let index = char_cursor.index();
                let c = grapheme::char_at(index, &text);

                has_uppercase = if grapheme::is_uppercase(c) {
                    true
                } else {
                    if has_uppercase {
                        text.insert(index + 1, ' ');
                    }

                    false
                };

                char_cursor.previous_boundary(&text);
            }

            command_palette.result_list.push(CommandPaletteResult {
                text,
                meta_data: CommandPaletteMetaData::ActionName(*action_name),
            });
        }
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
        _: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        let Some(CommandPaletteResult {
            meta_data: CommandPaletteMetaData::ActionName(action_name),
            ..
        }) = command_palette.result_list.get_focused()
        else {
            return CommandPaletteAction::Close;
        };

        args.ctx
            .window
            .actions_typed()
            .push(Action::from_name(*action_name));

        CommandPaletteAction::Close
    }
}
