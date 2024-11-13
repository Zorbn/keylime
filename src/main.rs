mod cursor;
mod doc;
mod editor;
mod gfx;
mod key;
mod keybind;
mod line_pool;
mod matrix;
mod mouse_button;
mod mousebind;
mod mouse_scroll;
mod position;
mod text;
mod visual_position;
mod window;

use editor::Editor;
use gfx::Color;
use line_pool::LinePool;
use window::Window;

/*
 * TODO:
 * Ctrl modifier for arrows, backspace, delete.
 * Common hotkeys: home, end, ctrl-a.
 * Scrolling: Mouse wheel, pgup, pgdn, arrows.
 * Undo/redo.
 * Multiple cursors.
 * Syntax highlighting.
 * Open/save/close/save-as dialogs.
 * Multiple tabs.
 * Multiple panes (split view).
 * File tree.
 * Per file type indentation.
 * Comment region: ctrl-/.
 * Indent-unindent region: ctrl-[, ctrl-], tab, shift-tab.
 * Copy/cut/paste.
 * Implicit copy/cut/paste for current line when shortcut is pressed but nothing is selected.
 * Support for OS scaling (eg. 125% applied for a monitor).
 * Search and search & replace.
 * Running commands and seeing output (very simple integrated terminal).
 * Find in files.
 * Configuration file: colors, fonts.
 * Command palette (eg. for ctrl-g, go to line).
 * Simple auto-complete.
 */

fn main() {
    println!("Hello, world!");

    let mut line_pool = LinePool::new();

    let mut editor = Editor::new(&mut line_pool);

    let mut window = Window::new().unwrap();

    while window.is_running() {
        let _ = window.update();
        // println!("dt: {dt}");

        editor.update(&mut window, &mut line_pool);

        let gfx = window.gfx();

        gfx.begin_frame(Color::new(245, 245, 245, 255));

        editor.draw(gfx);

        gfx.end_frame();
    }
}
