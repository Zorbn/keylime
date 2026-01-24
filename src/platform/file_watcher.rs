use std::path::{Path, PathBuf};

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

    pub fn changed_files<'a>(
        &'a mut self,
        current_dir: &'a Path,
    ) -> impl Iterator<Item = Pooled<PathBuf>> + 'a {
        self.inner
            .changed_files()
            .iter()
            .flat_map(|path| path.normalized(current_dir))
    }
}
