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

pub struct DirWatchHandles {
    overlapped: OVERLAPPED,
    dir_handle: HANDLE,
    buffer: [u8; 1024],
    are_in_use: bool,
    path: PathBuf,
}

impl DirWatchHandles {
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

impl Drop for DirWatchHandles {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.overlapped.hEvent);
            let _ = CloseHandle(self.dir_handle);
        }
    }
}

pub struct FileWatcher {
    // Boxed so that the buffer and overlapped pointers passed to ReadDirectoryChangesW
    // remain valid when DirWatchHandles is moved into the dir_watch_handles list or reordered.
    #[allow(clippy::vec_box)]
    dir_watch_handles: Vec<Box<DirWatchHandles>>,
    changed_files: Vec<Pooled<PathBuf>>,
}

impl FileWatcher {
    pub fn new() -> Self {
        Self {
            dir_watch_handles: Vec::new(),
            changed_files: Vec::new(),
        }
    }

    pub unsafe fn handle_dir_update(&mut self, index: usize) -> Result<()> {
        let handles = &mut self.dir_watch_handles[index];

        let mut buffer_offset = 0;

        loop {
            let info =
                handles.buffer.as_mut_ptr().add(buffer_offset) as *const FILE_NOTIFY_INFORMATION;
            let info = &*info;

            let file_name = from_raw_parts(
                info.FileName.as_ptr(),
                info.FileNameLength as usize / size_of::<u16>(),
            );

            let mut path = PATH_POOL.new_item();
            path.push(&handles.path);
            path.push(String::from_utf16_lossy(file_name));

            if let Ok(path) = path.normalized() {
                self.changed_files.push(path);
            }

            buffer_offset += info.NextEntryOffset as usize;

            if info.NextEntryOffset == 0 {
                break;
            }
        }

        handles.enqueue()
    }

    pub unsafe fn check_dir_updates(&mut self) -> Result<()> {
        for i in 0..self.dir_watch_handles.len() {
            if WaitForSingleObject(self.dir_watch_handles[i].overlapped.hEvent, 0) != WAIT_OBJECT_0
            {
                continue;
            }

            self.handle_dir_update(i)?;
        }

        Ok(())
    }

    pub fn update<'a>(&mut self, files: impl Iterator<Item = &'a Path>) -> Result<()> {
        self.changed_files.clear();

        for handles in &mut self.dir_watch_handles {
            handles.are_in_use = false;
        }

        'docs: for file in files {
            let Some(dir) = file.parent() else {
                continue;
            };

            for handle in &mut self.dir_watch_handles {
                if dir.starts_with(&handle.path) {
                    handle.are_in_use = true;
                    continue 'docs;
                }
            }

            let handles = unsafe {
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

                let mut handles = Box::new(DirWatchHandles {
                    overlapped,
                    dir_handle,
                    buffer: [0; 1024],
                    are_in_use: true,
                    path: dir.to_path_buf(),
                });

                handles.enqueue()?;

                handles
            };

            self.dir_watch_handles.push(handles);
        }

        for i in (0..self.dir_watch_handles.len()).rev() {
            if !self.dir_watch_handles[i].are_in_use {
                self.dir_watch_handles.remove(i);
            }
        }

        Ok(())
    }

    pub fn changed_files(&mut self) -> &[Pooled<PathBuf>] {
        &self.changed_files
    }

    pub fn dir_watch_handles(&self) -> &[Box<DirWatchHandles>] {
        &self.dir_watch_handles
    }
}
