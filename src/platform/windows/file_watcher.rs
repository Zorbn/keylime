use std::{
    path::{Path, PathBuf},
    slice::from_raw_parts,
};

use windows::{
    core::{Result, HSTRING},
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
        Storage::FileSystem::*,
        System::{
            Threading::{CreateEventW, WaitForSingleObject},
            IO::OVERLAPPED,
        },
    },
};

use crate::{
    normalizable::Normalizable,
    pool::{Pooled, PATH_POOL},
};

pub struct WatchedDir {
    overlapped: OVERLAPPED,
    dir_handle: HANDLE,
    buffer: [u8; 1024],
    is_in_use: bool,
    path: Pooled<PathBuf>,
}

impl WatchedDir {
    unsafe fn enqueue(&mut self) -> Result<()> {
        ReadDirectoryChangesW(
            self.dir_handle,
            self.buffer.as_mut_ptr() as _,
            self.buffer.len() as u32,
            true,
            FILE_NOTIFY_CHANGE_LAST_WRITE,
            None,
            Some(&mut self.overlapped),
            None,
        )
    }

    pub fn event(&self) -> HANDLE {
        self.overlapped.hEvent
    }
}

impl Drop for WatchedDir {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.overlapped.hEvent);
            let _ = CloseHandle(self.dir_handle);
        }
    }
}

pub struct FileWatcher {
    // Boxed so that the buffer and overlapped pointers passed to ReadDirectoryChangesW
    // remain valid when WatchedDir is moved into the watched_dirs list or reordered.
    #[allow(clippy::vec_box)]
    watched_dirs: Vec<Box<WatchedDir>>,
    changed_files: Vec<Pooled<PathBuf>>,
}

impl FileWatcher {
    pub fn new() -> Self {
        Self {
            watched_dirs: Vec::new(),
            changed_files: Vec::new(),
        }
    }

    pub unsafe fn handle_dir_update(&mut self, index: usize) -> Result<()> {
        let watched_dir = &mut self.watched_dirs[index];

        let mut buffer_offset = 0;

        loop {
            let info = watched_dir.buffer.as_mut_ptr().add(buffer_offset)
                as *const FILE_NOTIFY_INFORMATION;
            let info = &*info;

            let file_name = from_raw_parts(
                info.FileName.as_ptr(),
                info.FileNameLength as usize / size_of::<u16>(),
            );

            let mut path = PATH_POOL.new_item();
            path.push(&watched_dir.path);
            path.push(String::from_utf16_lossy(file_name));

            if let Ok(path) = path.normalized() {
                self.changed_files.push(path);
            }

            buffer_offset += info.NextEntryOffset as usize;

            if info.NextEntryOffset == 0 {
                break;
            }
        }

        watched_dir.enqueue()
    }

    pub unsafe fn check_dir_updates(&mut self) -> Result<()> {
        for i in 0..self.watched_dirs.len() {
            if WaitForSingleObject(self.watched_dirs[i].overlapped.hEvent, 0) != WAIT_OBJECT_0 {
                continue;
            }

            self.handle_dir_update(i)?;
        }

        Ok(())
    }

    pub fn update<'a>(&mut self, files: impl Iterator<Item = &'a Path>) -> Result<()> {
        self.changed_files.clear();

        self.watched_dirs.sort_by(|a, b| {
            let a = a.path.as_os_str();
            let b = b.path.as_os_str();

            a.len().cmp(&b.len())
        });

        for watched_dir in &mut self.watched_dirs {
            watched_dir.is_in_use = false;
        }

        'docs: for file in files {
            let Some(dir) = file.parent() else {
                continue;
            };

            for watched_dir in &mut self.watched_dirs {
                if dir.starts_with(&watched_dir.path) {
                    watched_dir.is_in_use = true;
                    continue 'docs;
                }
            }

            let watched_dir = unsafe {
                let path = HSTRING::from(dir.as_os_str());

                let overlapped = OVERLAPPED {
                    hEvent: CreateEventW(None, false, false, None)?,
                    ..Default::default()
                };

                let dir_handle = CreateFileW(
                    &path,
                    FILE_LIST_DIRECTORY.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
                    None,
                )?;

                let mut watched_dir = Box::new(WatchedDir {
                    overlapped,
                    dir_handle,
                    buffer: [0; 1024],
                    is_in_use: true,
                    path: dir.into(),
                });

                watched_dir.enqueue()?;

                watched_dir
            };

            self.watched_dirs.push(watched_dir);
        }

        self.watched_dirs
            .retain(|watched_dir| watched_dir.is_in_use);

        Ok(())
    }

    pub fn changed_files(&mut self) -> &[Pooled<PathBuf>] {
        &self.changed_files
    }

    pub fn watched_dirs(&self) -> &[Box<WatchedDir>] {
        &self.watched_dirs
    }
}
