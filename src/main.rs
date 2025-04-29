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
 * Scroll bar along the right side of each tab that shows the current camera location as well as the location of diagnostics.
 * Language server completion responses might be a list of completion items instead of a completion list object.
 * Skip adding lsp completion results if by the time they are being added the popup shouldn't be open anymore or the current doc was changed.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
