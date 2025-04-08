mod app;
mod config;
mod digits;
mod editor_buffers;
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
 * Directory-wide search.
 * Unit testing for patterns and text editing functions.
 * 🏼🏼🏼.
 * Ctrl-R to reverse search in the terminal has broken rendering.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
