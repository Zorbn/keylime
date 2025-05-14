use crate::ui::result_list::ResultListSubmitKind;

use super::{
    find_in_files_mode::FindInFilesMode,
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction, CommandPaletteResult,
};

pub struct ReferencesMode {
    results: Vec<CommandPaletteResult>,
}

impl ReferencesMode {
    pub fn new(results: Vec<CommandPaletteResult>) -> Self {
        Self { results }
    }
}

impl CommandPaletteMode for ReferencesMode {
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
        args: CommandPaletteEventArgs,
        kind: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        FindInFilesMode::jump_to_path_with_position(command_palette, args, kind)
    }
}
