#![cfg_attr(test, allow(dead_code))]
#![warn(clippy::redundant_closure_for_method_calls)]
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
 * Add action list command palette mode.
 * If the language server returns no completions for a request use simple completions instead.
 */

fn main() {
    println!("Hello, world!");

    run_app().unwrap();
}
