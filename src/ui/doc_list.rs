use std::{io, path::Path};

use crate::{
    platform::dialog::{find_file, message, FindFileKind, MessageKind, MessageResponse},
    text::{
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
};

pub struct DocList {
    docs: Vec<Option<Doc>>,
}

impl DocList {
    pub fn new() -> Self {
        Self { docs: Vec::new() }
    }

    pub fn add(&mut self, doc: Doc) -> usize {
        let mut index = None;

        for i in 0..self.docs.len() {
            if self.docs[i].is_none() {
                index = Some(i);
                break;
            }
        }

        if let Some(index) = index {
            self.docs[index] = Some(doc);
            index
        } else {
            self.docs.push(Some(doc));
            self.docs.len() - 1
        }
    }

    pub fn remove(&mut self, index: usize, line_pool: &mut LinePool) {
        let Some(doc) = self.get_mut(index) else {
            return;
        };

        doc.clear(line_pool);

        self.docs[index] = None;
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Doc> {
        self.docs.get_mut(index).and_then(|doc| doc.as_mut())
    }

    pub fn confirm_close_all(&mut self, reason: &str, line_pool: &mut LinePool, time: f32) {
        for doc in self.docs.iter_mut().filter_map(|doc| doc.as_mut()) {
            Self::confirm_close(doc, reason, false, line_pool, time);
        }
    }

    pub fn confirm_close(
        doc: &mut Doc,
        reason: &str,
        is_cancelable: bool,
        line_pool: &mut LinePool,
        time: f32,
    ) -> bool {
        if doc.is_saved() {
            true
        } else {
            let text = format!(
                "{} has unsaved changes. Do you want to save it before {}?",
                doc.file_name(),
                reason
            );

            let message_kind = if is_cancelable {
                MessageKind::YesNoCancel
            } else {
                MessageKind::YesNo
            };

            match message("Unsaved Changes", &text, message_kind) {
                MessageResponse::Yes => Self::try_save(doc, line_pool, time),
                MessageResponse::No => true,
                MessageResponse::Cancel => false,
            }
        }
    }

    pub fn try_save(doc: &mut Doc, line_pool: &mut LinePool, time: f32) -> bool {
        let path = if let Some(path) = doc.path() {
            Ok(path.to_owned())
        } else {
            find_file(FindFileKind::Save)
        };

        let Ok(path) = path else {
            return false;
        };

        doc.trim_trailing_whitespace(line_pool, time);

        if let Err(err) = doc.save(path) {
            message("Failed to Save File", &err.to_string(), MessageKind::Ok);
            false
        } else {
            true
        }
    }

    pub fn reload(doc: &mut Doc, line_pool: &mut LinePool, time: f32) {
        if !Self::confirm_close(doc, "reloading", true, line_pool, time) {
            return;
        }

        let Some(path) = doc.path().map(|path| path.to_owned()) else {
            return;
        };

        if let Err(err) = doc.load(&path, line_pool) {
            message("Failed to Reload File", &err.to_string(), MessageKind::Ok);
        }
    }

    pub fn open_or_reuse(&mut self, path: &Path, line_pool: &mut LinePool) -> io::Result<usize> {
        for (i, doc) in self.docs.iter().filter_map(|doc| doc.as_ref()).enumerate() {
            if doc.path() == Some(path) {
                return Ok(i);
            }
        }

        let mut doc = Doc::new(line_pool, DocKind::MultiLine);

        doc.load(path, line_pool)?;

        Ok(self.add(doc))
    }
}
