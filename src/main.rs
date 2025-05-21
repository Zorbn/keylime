#![cfg_attr(test, allow(dead_code))]
#![warn(clippy::use_self)]

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
 * Now that SlotIds can be used to permanently identify docs, check for places where other identifies are being used (eg. Paths) and replace them with SlotIds if possible.
 * If the language server returns no completions for a request use simple completions instead.
 * Bounds could instead be accessed with a ui.bounds(widget_id) fn and set with a ui.layout(widget_id, bounds) fn.
 * - The same could be applied to doc cursors, there is a lot of doc.cursor(...).position or doc.cursor(...).position =, etc.
 * Support multiple terminal panes.
 * Rust analyzer format on save pushes cursor to the bottom of the doc on Windows.
 * Closing crashes on Windows?
 */

fn main() {
    println!("Hello, world!");

    run_app().unwrap();
}
