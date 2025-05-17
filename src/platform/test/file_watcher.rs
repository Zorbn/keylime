use std::path::PathBuf;

use crate::pool::Pooled;

pub struct FileWatcher;

impl FileWatcher {
    pub fn new() -> Self {
        Self
    }

    pub fn changed_files(&self) -> &[Pooled<PathBuf>] {
        &[]
    }
}
