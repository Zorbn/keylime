#![cfg_attr(test, allow(dead_code))]

mod app;
mod bit_field;
mod config;
mod ctx;
mod digits;
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
 * Add LSP hover documentation support.
 * Consider renaming DocKind::Output to DocKind::Raw
 * Consider making DocKinds predefined BitFields that store a list of features such action history, position shifting, multi line, etc.
 * Try to use Pooled items for deserialization.
 * Replace String::new and PathBuf::new with Pooled usages. Some functions like the ones for URIs can be simplified to not accept a mutable String/PathBuf since we can just grab one from the pool.
 * - Basically search for anything with buffer in the name, might be useless or need to be renamed to reflect that it isn't from EditorBuffers any more.
 * Update windows platform to use Pooled items where necessary.
 */

fn main() {
    println!("Hello, world!");

    run_app().unwrap();
}
