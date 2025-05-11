#![cfg_attr(test, allow(dead_code))]

mod app;
mod bit_field;
mod config;
mod ctx;
mod digits;
mod editor_buffers;
mod geometry;
mod input;
mod lsp;
mod normalizable;
mod platform;
mod temp_buffer;
mod text;
mod ui;

#[cfg(test)]
mod tests;

use platform::app_runner::run_app;

/*
 * TODO:
 * If the language server returns no completions for a request use simple completions instead.
 * Add LSP hover documentation support.
 * Consider renaming DocKind::Output to DocKind::Raw
 * Consider making DocKinds predefined BitFields that store a list of features such action history, position shifting, multi line, etc.
 */

fn main() {
    println!("Hello, world!");

    run_app().unwrap();
}
