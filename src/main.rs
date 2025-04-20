#![cfg_attr(test, allow(dead_code))]

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

#[cfg(test)]
mod tests;

use app::App;
use platform::window::WindowRunner;

/*
 * TODO:
 * Ensure font is monospaced on Windows.
 * Use incremental searching for the find in files menu. No more than a certain number of steps/time per frame, check between every search and every dir entry.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
