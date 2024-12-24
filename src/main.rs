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
 * Support unicode:
 *  - For ASCII characters a hashmap lookup isn't needed to determine their atlas offset,
 * Support unicode on Windows:
 *  - Pty,
 *  - Text/Gfx
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
