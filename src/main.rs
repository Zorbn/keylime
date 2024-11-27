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
 *    - Allow focusing either the terminal or the editor using the mouse and keybinds,
 *    - Support ctrl-c/v to and from terminal,
 *    - Add colors for output,
 *    - Have a list of fallback shells (if pwsh isn't available try powershell, then cmd, etc).
 * More command palette commands (open folder, new file/folder, recycle file/folder, etc).
 * Directory-wide search.
 * Unit testing for patterns and text editing functions.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
