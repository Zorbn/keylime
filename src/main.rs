#![cfg_attr(test, allow(dead_code))]

mod app;
mod bit_field;
mod config;
mod ctx;
mod geometry;
mod input;
mod lsp;
mod normalizable;
mod platform;
mod pool;
mod text;
mod ui;

#[cfg(test)]
mod tests;

use platform::app_runner::run_app;

/*
 * TODO:
 * If the language server returns no completions for a request use simple completions instead.
 * Layout system that allows widgets to have children.
 * - Necessary for mouse hover support, because we need to be sure that the mouse is hovering over a doc and not a popup or completion list.
 * - Remove the capability for widgets to have multiple bounding boxes, replace that with children.
 * - Can we handle layout on demand? Requirement for this is platform changes to support a resize callback on the app and gfx/window/time being passed to new
 *   For making the window the correct theme on startup, either:
 *   - The initial config needs to be passed in to run_app
 *   - Or, when a window is created don't show it until the first draw (i think we already do this), and we call App::new then grab app.config() and set the theme accordingly.
 * Add LSP mouse hover documentation/diagnostic support.
 * Consider renaming DocKind::Output to DocKind::Raw
 * Consider making DocKinds predefined BitFields that store a list of features such action history, position shifting, multi line, etc.
 */

fn main() {
    println!("Hello, world!");

    run_app().unwrap();
}
