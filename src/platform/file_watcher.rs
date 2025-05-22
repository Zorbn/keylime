use std::path::PathBuf;

use crate::{normalizable::Normalizable, pool::Pooled};

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

    pub fn changed_files(&mut self) -> &[Pooled<PathBuf>] {
        let changed_files = self.inner.changed_files();

        assert!(changed_files.iter().all(Normalizable::is_normal));

        changed_files
    }
}
