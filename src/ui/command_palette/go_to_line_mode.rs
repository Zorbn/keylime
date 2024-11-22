use crate::{
    geometry::position::Position,
    text::line_pool::LinePool,
    ui::{camera::CameraRecenterKind, doc_list::DocList, pane::Pane},
};

use super::{mode::CommandPaletteMode, CommandPalette, CommandPaletteAction};

pub const MODE_GO_TO_LINE: &CommandPaletteMode = &CommandPaletteMode {
    title: "Go to Line",
    on_submit: on_submit_go_to_line,
    ..CommandPaletteMode::default()
};

fn on_submit_go_to_line(
    command_palette: &mut CommandPalette,
    _: bool,
    pane: &mut Pane,
    doc_list: &mut DocList,
    _: &mut LinePool,
    _: f32,
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
    tab.camera.vertical.recenter(CameraRecenterKind::OnCursor);
    tab.camera
        .horizontal
        .recenter(CameraRecenterKind::OnScrollBorder);

    CommandPaletteAction::Close
}
