mod app;
mod config;
mod ctx;
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
 * Terminal camera scrolls up slightly as soon as the editor starts, it should already be in place.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
