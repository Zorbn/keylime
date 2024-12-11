use std::path::PathBuf;

pub struct FileWatcher {}

impl FileWatcher {
    pub fn changed_files(&self) -> &[PathBuf] {
        &[]
    }
}
