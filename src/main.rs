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
 * Fallback to reasonable fonts if the one in the config can't be loaded.
 * MacOS support:
 * - Refactor code that could be shared between platforms, eg. Gfx::measure_text and similar,
 *   Gfx::add_bordered_rect, Gfx::add_rect, Gfx::add_text?
 *
 * - Possibly all of the types/functions from the platform module should just be wrappers that
 *   wrap different internal implementations depending on the platform? eg. Window wraps WindowsWindow and MacOSWindow.
 *
 * - Add support for per-platform keybinds (so that command and option work as expected).
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
