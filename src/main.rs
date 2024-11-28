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
 *    - Allow background syntax highlighting for use in the terminal (instead of supporting foreground colors only),
 *    - Move terminal colors to theme and support bright versions of terminal colors (eg. see "-Recurse"'s color in "Get-ChildItem *.rs -Recurse | Select-String "fallback""),
 *    - Resize the pty when the size of the terminal changes,
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
