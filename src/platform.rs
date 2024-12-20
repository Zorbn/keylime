#[cfg(target_os = "windows")]
#[path = "platform/windows/mod.rs"]
mod platform_impl;

#[cfg(target_os = "macos")]
#[path = "platform/macos/mod.rs"]
mod platform_impl;

pub mod dialog;
pub mod file_watcher;
pub mod gfx;
pub mod pty;
pub mod result;
mod text;
pub mod window;