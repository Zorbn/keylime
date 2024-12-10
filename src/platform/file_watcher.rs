use std::{
    path::{Path, PathBuf},
    slice::from_raw_parts,
};

use windows::{
    core::{Result, HSTRING},
    Win32::{
        Foundation::{CloseHandle, FALSE, HANDLE, TRUE, WAIT_OBJECT_0},
        Storage::FileSystem::*,
        System::{
            Threading::{CreateEventW, WaitForSingleObject},
            IO::OVERLAPPED,
        },
    },
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
            self.buffer.as_ptr() as _,
            self.buffer.len() as u32,
            TRUE,
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
    dir_watch_handles: Vec<DirWatchHandles>,
    changed_files: Vec<PathBuf>,
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

            let mut path = PathBuf::new();
            path.push(&handles.path);
            path.push(String::from_utf16_lossy(file_name));

            self.changed_files.push(path);

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
                    hEvent: CreateEventW(None, FALSE, None, None)?,
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

                let mut handles = DirWatchHandles {
                    overlapped,
                    dir_handle,
                    buffer: [0; 1024],
                    are_in_use: true,
                    path: dir.to_path_buf(),
                };

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

    pub fn changed_files(&self) -> &[PathBuf] {
        &self.changed_files
    }

    pub fn dir_watch_handles(&self) -> &[DirWatchHandles] {
        &self.dir_watch_handles
    }
}
