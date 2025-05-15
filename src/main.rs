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
 * - Make popups for completion list docs/labels and examine/signature help popups part of the widget tree.
 * - Necessary for mouse hover support, because we need to be sure that the mouse is hovering over a doc and not a popup or completion list.
 * Add LSP mouse hover documentation/diagnostic support.
 * Consider renaming DocKind::Output to DocKind::Raw
 * Consider making DocKinds predefined BitFields that store a list of features such action history, position shifting, multi line, etc.
 */

fn main() {
    println!("Hello, world!");

    run_app().unwrap();
}
