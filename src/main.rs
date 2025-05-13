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
 * Make performing completions work for all cursors when using multicursor?
 * Add a command palette mode that lists all diagnostics and lets you jump to them.
 * Add LSP hover documentation/diagnostic support.
 * - Also, rename ShowDiagnostic to something else and make it so that it shows documentation if there is no diagnostic at the cursor.
 * Consider renaming DocKind::Output to DocKind::Raw
 * Consider making DocKinds predefined BitFields that store a list of features such action history, position shifting, multi line, etc.
 */

fn main() {
    println!("Hello, world!");

    run_app().unwrap();
}
