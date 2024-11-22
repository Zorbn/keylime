use crate::geometry::position::Position;

use super::{
    mode::{CommandPaletteEventArgs, CommandPaletteMode},
    CommandPaletteAction,
};

pub const MODE_GO_TO_LINE: &CommandPaletteMode = &CommandPaletteMode {
    title: "Go to Line",
    on_submit: on_submit_go_to_line,
    ..CommandPaletteMode::default()
};

fn on_submit_go_to_line(
    CommandPaletteEventArgs {
        command_palette,
        pane,
        doc_list,
        ..
    }: CommandPaletteEventArgs,
    _: bool,
) -> CommandPaletteAction {
    let focused_tab_index = pane.focused_tab_index();

    let line_text = command_palette.doc.to_string();

    let Ok(line) = str::parse::<isize>(&line_text) else {
        return CommandPaletteAction::Close;
    };

    let Some((tab, doc)) = pane.get_tab_with_doc(focused_tab_index, doc_list) else {
        return CommandPaletteAction::Close;
    };

    doc.jump_cursors(Position::new(0, line), false);
    tab.camera.recenter();

    CommandPaletteAction::Close
}
