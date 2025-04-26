#[cfg(all(target_os = "windows", not(test)))]
#[path = "platform/windows/mod.rs"]
mod platform_impl;

#[cfg(all(target_os = "macos", not(test)))]
#[path = "platform/macos/mod.rs"]
mod platform_impl;

#[cfg(test)]
#[path = "platform/test/mod.rs"]
mod platform_impl;

mod aliases;
pub mod dialog;
pub mod file_watcher;
pub mod gfx;
pub mod process;
pub mod recycle;
pub mod result;
mod text;
mod text_cache;
pub mod window;
