#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

mod app;
mod config;
mod digits;
mod geometry;
mod input;
mod platform;
mod temp_buffer;
mod text;
mod ui;

use app::App;
use platform::window::WindowRunner;

/*
 * TODO:
 * More command palette commands (open folder, new file/folder, recycle file/folder, etc).
 * Directory-wide search.
 * Unit testing for patterns and text editing functions.
 * Add support for per-platform keybinds (so that command and option work as expected).
 * Fix case where cursor cannot be kept out of the scroll border because there aren't enough visible lines (eg. set font size to 24.0).
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
