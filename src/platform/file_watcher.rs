use std::path::PathBuf;

use crate::normalizable::Normalizable;

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
        let changed_files = self.inner.get_changed_files();

        assert!(changed_files.iter().all(|path| path.is_normal()));

        changed_files
    }
}
