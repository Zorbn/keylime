use crate::{command_palette::CommandPalette, editor::Editor, line_pool::LinePool};

use super::CommandPaletteAction;

#[derive(Clone, Copy)]
pub struct CommandPaletteMode {
    pub title: &'static str,
    pub on_submit:
        fn(&mut CommandPalette, bool, &mut Editor, &mut LinePool, f32) -> CommandPaletteAction,
    pub on_complete_result: fn(&mut CommandPalette, &mut LinePool, f32),
    pub on_update_results: fn(&mut CommandPalette, &mut LinePool, f32),
    pub on_backspace: fn(&mut CommandPalette, &mut LinePool, f32) -> bool,
    pub do_passthrough_result: bool,
}
