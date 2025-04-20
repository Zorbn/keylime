use crate::{
    ctx::Ctx,
    text::doc::Doc,
    ui::{
        editor::{editor_pane::EditorPane, Editor},
        result_list::ResultListSubmitKind,
        slot_list::SlotList,
    },
};

use super::{CommandPalette, CommandPaletteAction};

pub struct CommandPaletteEventArgs<'a, 'b> {
    pub pane: &'a mut EditorPane,
    pub doc_list: &'a mut SlotList<Doc>,
    pub ctx: &'a mut Ctx<'b>,
}

impl<'a, 'b> CommandPaletteEventArgs<'a, 'b> {
    pub fn new(editor: &'a mut Editor, ctx: &'a mut Ctx<'b>) -> CommandPaletteEventArgs<'a, 'b> {
        let (pane, doc_list) = editor.get_focused_pane_and_doc_list();

        CommandPaletteEventArgs {
            pane,
            doc_list,
            ctx,
        }
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
