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
    pub time: f32,
}

impl<'a, 'b> CommandPaletteEventArgs<'a, 'b> {
    pub fn new(
        editor: &'a mut Editor,
        ctx: &'a mut Ctx<'b>,
        time: f32,
    ) -> CommandPaletteEventArgs<'a, 'b> {
        let (pane, doc_list) = editor.get_focused_pane_and_doc_list();

        CommandPaletteEventArgs {
            pane,
            doc_list,
            ctx,
            time,
        }
    }
}

pub struct CommandPaletteMode {
    pub title: &'static str,
    pub on_open: fn(&mut CommandPalette, CommandPaletteEventArgs),
    pub on_submit: fn(
        &mut CommandPalette,
        CommandPaletteEventArgs,
        ResultListSubmitKind,
    ) -> CommandPaletteAction,
    pub on_complete_result: fn(&mut CommandPalette, CommandPaletteEventArgs),
    pub on_update_results: fn(&mut CommandPalette, CommandPaletteEventArgs),
    pub on_backspace: fn(&mut CommandPalette, CommandPaletteEventArgs) -> bool,
    pub do_passthrough_result: bool,
}

impl CommandPaletteMode {
    pub const fn default() -> Self {
        Self {
            title: "Unnamed",
            on_open: |_, _| {},
            on_submit: |_, _, _| CommandPaletteAction::Stay,
            on_complete_result: |_, _| {},
            on_update_results: |_, _| {},
            on_backspace: |_, _| false,
            do_passthrough_result: false,
        }
    }
}
