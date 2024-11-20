use crate::{command_palette::CommandPalette, editor::Editor, line_pool::LinePool};

use super::CommandPaletteAction;

#[derive(Clone, Copy)]
pub struct CommandPaletteMode {
    pub title: &'static str,
    pub on_open: fn(&mut CommandPalette, &mut Editor, &mut LinePool, f32),
    pub on_submit:
        fn(&mut CommandPalette, bool, &mut Editor, &mut LinePool, f32) -> CommandPaletteAction,
    pub on_complete_result: fn(&mut CommandPalette, &mut LinePool, f32),
    pub on_update_results: fn(&mut CommandPalette, &mut LinePool, f32),
    pub on_backspace: fn(&mut CommandPalette, &mut LinePool, f32) -> bool,
    pub do_passthrough_result: bool,
}

impl CommandPaletteMode {
    pub const fn default() -> Self {
        Self {
            title: "Unnamed",
            on_open: |_, _, _, _| {},
            on_submit: |_, _, _, _, _| CommandPaletteAction::Stay,
            on_complete_result: |_, _, _| {},
            on_update_results: |_, _, _| {},
            on_backspace: |_, _, _| false,
            do_passthrough_result: false,
        }
    }
}
