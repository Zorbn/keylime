#![cfg_attr(test, allow(dead_code))]

mod app;
mod bit_field;
mod config;
mod ctx;
mod digits;
mod editor_buffers;
mod geometry;
mod input;
mod lsp;
mod platform;
mod temp_buffer;
mod text;
mod ui;

#[cfg(test)]
mod tests;

use app::App;
use platform::window::WindowRunner;

/*
 * TODO:
 * Auto indent should follow the same logic as the indent guides to determine width.
 * While typing in the search box, interactively search from the original start position to the first result.
 * Cmd-T search for tab, editor, etc and see that the ideal top result is 2nd for some reason.
 * Add status bar at the bottom of the editor (above the terminal) with misc info.
 * When reloading a file compare the lines and only delete & re-insert doc lines that changed.
 * Use FocusList for ResultList's results/selection.
 * If the language server returns no completions for a request use simple completions instead.
 * Add LSP signature help support.
 * Add LSP hover documentation support.
 * Consider renaming DocKind::Output to DocKind::Raw
 * Consider making DocKinds predefined BitFields that store a list of features such action history, position shifting, multi line, etc.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
