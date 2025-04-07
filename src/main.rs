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
 * Correctly render complex emojis like 🧑🏼‍🦳.
 * 🏼 Breaks the terminal always and the text editor if the fast path for ascii is removed from GraphemeCursor.
 * ^ Because this character is a modifier so it has strange behavior.
 * Terminal color doesn't currently work and making a line too long in the terminal also breaks.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
