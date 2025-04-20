use std::path::PathBuf;

pub struct FileWatcher;

impl FileWatcher {
    pub fn new() -> Self {
        Self
    }

    pub fn get_changed_files(&mut self) -> &[PathBuf] {
        &[]
    }
}
