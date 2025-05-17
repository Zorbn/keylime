use crate::{pool::Pooled, ui::result_list::ResultListSubmitKind};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction,
};

pub struct RenameMode {
    placeholder: Pooled<String>,
}

impl RenameMode {
    pub fn new(placeholder: Pooled<String>) -> Self {
        Self { placeholder }
    }
}

impl CommandPaletteMode for RenameMode {
    fn title(&self) -> &str {
        "Rename"
    }

    fn on_open(&mut self, command_palette: &mut CommandPalette, args: CommandPaletteEventArgs) {
        let command_doc = &mut command_palette.doc;

        command_doc.insert(command_doc.end(), &self.placeholder, args.ctx);
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
        _: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        let (pane, doc_list) = args.editor.last_focused_pane_and_doc_list(args.ctx.ui);
        let focused_tab_index = pane.focused_tab_index();
        let input = command_palette.input();

        let Some((_, doc)) = pane.get_tab_with_data(focused_tab_index, doc_list) else {
            return CommandPaletteAction::Close;
        };

        doc.lsp_rename(input, args.ctx);

        CommandPaletteAction::Close
    }
}
