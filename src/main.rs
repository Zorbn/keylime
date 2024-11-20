#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

mod action_history;
mod app;
mod camera;
mod char_category;
mod command_palette;
mod cursor;
mod cursor_index;
mod deferred_call;
mod dialog;
mod doc;
mod editor;
mod gfx;
mod input_handlers;
mod key;
mod keybind;
mod line_pool;
mod matrix;
mod mouse_button;
mod mouse_scroll;
mod mousebind;
mod position;
mod rect;
mod selection;
mod side;
mod syntax_highlighter;
mod tab;
mod temp_buffer;
mod text;
mod theme;
mod visual_position;
mod window;

use app::App;
use window::WindowRunner;

/*
 * TODO:
 * Multiple panes (split view).
 * Per file type indentation.
 * Comment region: ctrl-/.
 * Indent-unindent region: ctrl-[, ctrl-], tab, shift-tab.
 * Running commands and seeing output (very simple integrated terminal).
 * Configuration file: colors, fonts.
 * More command palette commands (open folder, new file/folder, recycle file/folder, etc).
 * Simple auto-complete.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
