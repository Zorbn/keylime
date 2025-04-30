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
 * Update Windows platform with input changes.
 * Undoing after reloading still does a delete/insert of the entire document so it will move cursor and diagnostics the the end of the file.
 * Add LSP code actions.
 * - Add support for the "codeActionLiteralSupport" capability.
 * - Put "isPreferred: true" code action on top of results.
 * Add LSP rename.
 * Add LSP find all references.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();
    let a = 5;

    window.run();
}
