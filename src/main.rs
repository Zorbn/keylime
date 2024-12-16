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
 *   Gfx::add_bordered_rect, Gfx::add_rect, Gfx::add_text?
 *
 * - Possibly all of the types/functions from the platform module should just be wrappers that
 *   wrap different internal implementations depending on the platform? eg. Window wraps WindowsWindow and MacOSWindow.
 *
 * - Figure out how best to architect the MacOS window/graphics class? Possibly AppKit/MetalKit delegates and subclasses
 *   should just be used to call into Window/Gfx where the real logic is performed.
 *
 * - Figure out when updates/redraws should be triggered. On windows we just do an update/redraw whenever any message is received,
 *   because that's easy to do when you control the main loop. For MacOS it seems like it will make more sense to explicitly
 *   trigger updates and redraws from each event. To handle animating we could have a function that does one update/draw and then
 *   continues doing them until is_animating is false. Evaluate this architecture compared to the Windows one, maybe the Windows
 *   implementation should change to match.
 *
 * - Rename Windows' Window::dpi() to Window::scale() which is more accurate.
 */

fn main() {
    println!("Hello, world!");

    let app = App::new();
    let mut window = WindowRunner::new(app).unwrap();

    window.run();
}
