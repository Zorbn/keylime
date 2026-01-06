use crate::{
    input::editing_actions::handle_select_all, pool::Pooled, ui::result_list::ResultListSubmitKind,
};

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
        let CommandPalette { doc, tab, .. } = command_palette;

        doc.insert(doc.end(), &self.placeholder, args.ctx);
        handle_select_all(doc, args.ctx.gfx);
        tab.skip_cursor_animations(doc, args.ctx);
    }

    fn on_submit(
        &mut self,
        command_palette: &mut CommandPalette,
        args: CommandPaletteEventArgs,
        _: ResultListSubmitKind,
    ) -> CommandPaletteAction {
        let (pane, doc_list) = args.editor.last_focused_pane_and_doc_list(args.ctx.ui);

        let Some((_, doc)) = pane.get_focused_tab_with_data(doc_list) else {
            return CommandPaletteAction::Close;
        };

        let input = command_palette.input();
        doc.lsp_rename(input, args.ctx);

        CommandPaletteAction::Close
    }
}
