use std::{
    ffi::{c_void, CStr},
    path::{Path, PathBuf},
    ptr::NonNull,
    slice::from_raw_parts,
};

use dispatch2::DispatchQueue;
use objc2::rc::Weak;
use objc2_core_foundation::{CFArray, CFString};
use objc2_core_services::*;

use crate::pool::Pooled;

use super::view::{View, ViewRef};

struct WatchedDir {
    stream: FSEventStreamRef,
    path: Pooled<PathBuf>,
    is_in_use: bool,
}

impl Drop for WatchedDir {
    fn drop(&mut self) {
        unsafe {
            FSEventStreamInvalidate(self.stream);
            FSEventStreamRelease(self.stream);
        }
    }
}

struct CallbackInfo {
    changed_files: Vec<Pooled<PathBuf>>,
    view: ViewRef,
}

pub struct FileWatcher {
    watched_dirs: Vec<WatchedDir>,
    callback_info: Box<CallbackInfo>,
}

impl FileWatcher {
    pub fn new() -> Self {
        Self {
            watched_dirs: Vec::new(),
            callback_info: Box::new(CallbackInfo {
                changed_files: Vec::new(),
                view: ViewRef::default(),
            }),
        }
    }

    pub fn update<'a>(&mut self, files: impl Iterator<Item = &'a Path>, view: &Weak<View>) {
        if self.callback_info.view.is_none() {
            self.callback_info.view = ViewRef::new(view);
        }

        self.callback_info.changed_files.clear();

        self.watched_dirs.sort_by(|a, b| {
            let a = a.path.as_os_str();
            let b = b.path.as_os_str();

            a.len().cmp(&b.len())
        });

        for watched_path in &mut self.watched_dirs {
            watched_path.is_in_use = false;
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

            let mut callback_ctx = FSEventStreamContext {
                version: 0,
                info: self.callback_info.as_mut() as *mut _ as _,
                retain: None,
                release: None,
                copyDescription: None,
            };

            let paths =
                CFArray::from_retained_objects(&[CFString::from_str(dir.to_str().unwrap())]);

            let stream = unsafe {
                FSEventStreamCreate(
                    None,
                    Some(Self::callback),
                    &mut callback_ctx,
                    paths.as_ref(),
                    kFSEventStreamEventIdSinceNow,
                    0.0,
                    kFSEventStreamCreateFlagFileEvents,
                )
            };

            unsafe {
                FSEventStreamSetDispatchQueue(stream, Some(DispatchQueue::main()));
                assert!(FSEventStreamStart(stream));
            }

            self.watched_dirs.push(WatchedDir {
                stream,
                path: dir.into(),
                is_in_use: true,
            })
        }

        self.watched_dirs
            .retain(|watched_dir| watched_dir.is_in_use);
    }

    pub fn changed_files(&mut self) -> &[Pooled<PathBuf>] {
        &self.callback_info.changed_files
    }

    unsafe extern "C-unwind" fn callback(
        _stream_ref: ConstFSEventStreamRef,
        callback_info: *mut c_void,
        events_len: usize,
        event_paths: NonNull<c_void>,
        event_flags: NonNull<FSEventStreamEventFlags>,
        _event_ids: NonNull<FSEventStreamEventId>,
    ) {
        let callback_info = callback_info as *mut CallbackInfo;
        let callback_info = callback_info.as_mut().unwrap();

        let event_paths = event_paths.as_ptr() as *const *const i8;

        let event_paths = from_raw_parts(event_paths, events_len);
        let event_flags = from_raw_parts(event_flags.as_ptr(), events_len);

        let mut had_changes = false;

        for (path, flags) in event_paths.iter().zip(event_flags) {
            let is_file = flags & kFSEventStreamEventFlagItemIsFile == 0;
            let has_new_content = flags & kFSEventStreamEventFlagItemModified == 0
                && flags & kFSEventStreamEventFlagItemCreated == 0;

            if is_file && has_new_content {
                continue;
            }

            let path = CStr::from_ptr(*path);
            let path = path.to_str().unwrap();
            let path = Path::new(path);

            callback_info.changed_files.push(path.into());
            had_changes = true;
        }

        if had_changes {
            callback_info.view.update();
        }
    }
}
