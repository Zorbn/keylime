#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

mod app;
mod config;
mod deferred_call;
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
 * Running commands and seeing output (very simple integrated terminal).
 * More command palette commands (open folder, new file/folder, recycle file/folder, etc).
 * Simple auto-complete.
 *     - Fix prefix check, it currently skips trailing spaces leading to weird behavior when you press space with completion results,
 *     - Integrate auto complete results from multiple files,
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
