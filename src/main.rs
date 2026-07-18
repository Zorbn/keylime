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

use platform::{app_runner::run_app, result::Result};

// TODO: Fix tests.
// TODO: Visual indent should only affect full units of indentation, so that spaces after indentation for alignment are preserved (eg. one space before * in a block comment).
fn main() -> Result<()> {
    println!("Hello, world!");

    run_app()
}
