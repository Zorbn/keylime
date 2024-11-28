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
 *    - Have a list of fallback shells (if pwsh isn't available try powershell, then cmd, etc),
 *    - Move terminal colors to theme,
 *    - Add config option for terminal height in lines (excluding the tab bar),
 * More command palette commands (open folder, new file/folder, recycle file/folder, etc).
 * Directory-wide search.
 * Unit testing for patterns and text editing functions.
 * Double click on a word to select it.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
