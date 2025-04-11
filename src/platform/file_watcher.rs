use std::path::PathBuf;

use super::platform_impl;

pub struct FileWatcher {
    pub(super) inner: platform_impl::file_watcher::FileWatcher,
}

impl FileWatcher {
    pub fn new() -> Self {
        Self {
            inner: platform_impl::file_watcher::FileWatcher::new(),
        }
    }

    pub fn get_changed_files(&mut self) -> &[PathBuf] {
        self.inner.get_changed_files()
    }
}
