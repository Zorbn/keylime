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
 * Support copy/cut/paste in file finder. (Cut and paste trigger when you use the normal keybinds and have nothing selected, paste pastes a file if you have one copied).
 * - Files in the process of being cut should show up with the subtle color in the results list. (Maybe do this by expanding result_to_str to also return a color for ResultList::draw (display_result: fn(&'a T) -> (&'a str, Color)))
 * - Create ResultListActionKind for Delete, Copy, Paste, etc ops that are used by ResultListInput::Action and a command palette mode on_action event.
 * If the language server returns no completions for a request use simple completions instead.
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
