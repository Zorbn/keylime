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

// TODO: Visual indent doesn't work on highlighted text in some cases, eg. strings
// with leading whitespace, see the shader in windows/gfx.rs
fn main() -> Result<()> {
    println!("Hello, world!");

    run_app()
}
