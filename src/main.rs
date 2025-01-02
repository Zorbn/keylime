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
 * Allow recycling files/folders the selected folder in the file finder by pressing (forward) delete when the cursor is at the end of the line.
 * Directory-wide search.
 * Unit testing for patterns and text editing functions.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
