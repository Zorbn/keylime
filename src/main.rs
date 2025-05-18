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
 * Move cursor history from the doc to the editor to allow jumping back after doing go-to-definition.
 * If the language server returns no completions for a request use simple completions instead.
 * Bounds could instead be accessed with a ui.bounds(widget_id) fn and set with a ui.layout(widget_id, bounds) fn.
 * - The same could be applied to doc cursors, there is a lot of doc.cursor(...).position or doc.cursor(...).position =, etc.
 * Support multiple terminal panes.
 * Add LSP mouse hover documentation/diagnostic support.
 * - If the examine popup originated from a hover the examine popup should be shown even if the cursor is not visible.
 * Consider renaming DocKind::Output to DocKind::Raw
 * Consider making DocKinds predefined BitFields that store a list of features such action history, position shifting, multi line, etc.
 */

fn main() {
    println!("Hello, world!");

    run_app().unwrap();
}
