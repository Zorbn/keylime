use crate::{camera::CameraRecenterKind, editor::Editor, line_pool::LinePool, position::Position};

use super::{mode::CommandPaletteMode, CommandPalette, CommandPaletteAction};

pub const MODE_GO_TO_LINE: &CommandPaletteMode = &CommandPaletteMode {
    title: "Go to Line",
    on_submit: on_submit_go_to_line,
    ..CommandPaletteMode::default()
};

fn on_submit_go_to_line(
    command_palette: &mut CommandPalette,
    _: bool,
    editor: &mut Editor,
    _: &mut LinePool,
    _: f32,
) -> CommandPaletteAction {
    let focused_tab_index = editor.focused_tab_index();

    let line_text = command_palette.doc.to_string();

    let Ok(line) = str::parse::<isize>(&line_text) else {
        return CommandPaletteAction::Close;
    };

    let Some((tab, doc)) = editor.get_tab_with_doc(focused_tab_index) else {
        return CommandPaletteAction::Close;
    };

    doc.jump_cursors(Position::new(0, line), false);
    tab.camera.recenter(CameraRecenterKind::OnCursor);

    CommandPaletteAction::Close
}
