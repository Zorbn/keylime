#![cfg_attr(test, allow(dead_code))]

mod app;
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
 * Convert Pty to Process in Windows platform layer.
 * LSP process will be empty if command fails...
 * Handle what should happen when the user doesn't have a certain LSP installed.
 * Extend Process to allow supplying arguments to LSPs.
 * LSP handler that can run LSP servers for multiple languages within the current directory.
 * - Try to run a server when a file is opened that has a known language corresponding to a server that isn't running yet.
 * - Close all servers when changing working directory.
 * LSP diagnostics.
 * - Show a pop-up when hovering them or putting the cursor over them.
 * - If there are warnings or errors in a doc highlight it's name in the tab a corresponding color.
 * Scroll bar along the right side of each tab that shows the current camera location as well as the location of diagnostics.
 * LSP completions.
 * - Used instead of the tokenizer solution when a server is active for the current doc's language.
 * Replace TOML config with JSON config (the basic-toml library is unmaintained now anyway).
 * - Add JSON highlighter, keep TOML highlighter for Cargo.toml and stuff.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
