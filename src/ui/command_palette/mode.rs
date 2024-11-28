use crate::{
    config::Config,
    text::line_pool::LinePool,
    ui::{doc_list::DocList, editor_pane::EditorPane},
};

use super::{CommandPalette, CommandPaletteAction};

pub struct CommandPaletteEventArgs<'a> {
    pub command_palette: &'a mut CommandPalette,
    pub pane: &'a mut EditorPane,
    pub doc_list: &'a mut DocList,
    pub config: &'a Config,
    pub line_pool: &'a mut LinePool,
    pub time: f32,
}

pub struct CommandPaletteMode {
    pub title: &'static str,
    pub on_open: fn(CommandPaletteEventArgs),
    pub on_submit: fn(CommandPaletteEventArgs, bool) -> CommandPaletteAction,
    pub on_complete_result: fn(CommandPaletteEventArgs),
    pub on_update_results: fn(CommandPaletteEventArgs),
    pub on_backspace: fn(CommandPaletteEventArgs) -> bool,
    pub do_passthrough_result: bool,
}

impl CommandPaletteMode {
    pub const fn default() -> Self {
        Self {
            title: "Unnamed",
            on_open: |_| {},
            on_submit: |_, _| CommandPaletteAction::Stay,
            on_complete_result: |_| {},
            on_update_results: |_| {},
            on_backspace: |_| false,
            do_passthrough_result: false,
        }
    }
}
