use crate::{geometry::position::Position, ui::result_list::ResultListSubmitKind};

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPalette, CommandPaletteAction,
};

pub const MODE_GO_TO_LINE: &CommandPaletteMode = &CommandPaletteMode {
    title: "Go to Line",
    on_submit,
    ..CommandPaletteMode::default()
};

fn on_submit(
    command_palette: &mut CommandPalette,
    CommandPaletteEventArgs { pane, doc_list, .. }: CommandPaletteEventArgs,
    kind: ResultListSubmitKind,
) -> CommandPaletteAction {
    if kind != ResultListSubmitKind::Normal {
        return CommandPaletteAction::Stay;
    }

    let focused_tab_index = pane.focused_tab_index();

    let line_text = command_palette.doc.to_string();

    let Ok(line) = str::parse::<isize>(&line_text) else {
        return CommandPaletteAction::Close;
    };

    let Some((tab, doc)) = pane.get_tab_with_data_mut(focused_tab_index, doc_list) else {
        return CommandPaletteAction::Close;
    };

    doc.jump_cursors(Position::new(0, line), false);
    tab.camera.recenter();

    CommandPaletteAction::Close
}
