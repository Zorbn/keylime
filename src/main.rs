#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

#[cfg(target_os = "windows")]
#[path = "platform/windows/mod.rs"]
mod platform;

#[cfg(target_os = "macos")]
#[path = "platform/macos/mod.rs"]
mod platform;

mod app;
mod config;
mod deferred_call;
mod digits;
mod geometry;
mod input;
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
 * MacOS support:
 * - New platform module,
 * - Refactor code that could be shared between platforms, eg. Gfx::measure_text and similar,
 * - Possibly all of the types/functions from the platform module should just be wrappers that
 *   wrap different internal implementations depending on the platform? eg. Window wraps WindowsWindow and MacOSWindow.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
