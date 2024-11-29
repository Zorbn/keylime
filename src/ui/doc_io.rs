use std::{io, path::Path};

use crate::{
    config::Config,
    platform::dialog::{find_file, message, FindFileKind, MessageKind, MessageResponse},
    text::{
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
};

use super::slot_list::SlotList;

pub fn confirm_close(
    doc: &mut Doc,
    reason: &str,
    is_cancelable: bool,
    config: &Config,
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
            MessageResponse::Yes => try_save(doc, config, line_pool, time),
            MessageResponse::No => true,
            MessageResponse::Cancel => false,
        }
    }
}

pub fn try_save(doc: &mut Doc, config: &Config, line_pool: &mut LinePool, time: f32) -> bool {
    let path = if let Some(path) = doc.path() {
        Ok(path.to_owned())
    } else {
        find_file(FindFileKind::Save)
    };

    let Ok(path) = path else {
        return false;
    };

    if config.trim_trailing_whitespace {
        doc.trim_trailing_whitespace(line_pool, time);
    }

    if let Err(err) = doc.save(path) {
        message("Failed to Save File", &err.to_string(), MessageKind::Ok);
        false
    } else {
        true
    }
}

pub fn reload(doc: &mut Doc, config: &Config, line_pool: &mut LinePool, time: f32) {
    if !confirm_close(doc, "reloading", true, config, line_pool, time) {
        return;
    }

    let Some(path) = doc.path().map(|path| path.to_owned()) else {
        return;
    };

    if let Err(err) = doc.load(&path, line_pool) {
        message("Failed to Reload File", &err.to_string(), MessageKind::Ok);
    }
}

pub fn open_or_reuse(
    doc_list: &mut SlotList<Doc>,
    path: &Path,
    line_pool: &mut LinePool,
) -> io::Result<usize> {
    for (i, doc) in doc_list.iter().enumerate() {
        if doc.as_ref().and_then(|doc| doc.path()) == Some(path) {
            return Ok(i);
        }
    }

    let mut doc = Doc::new(line_pool, None, DocKind::MultiLine);

    doc.load(path, line_pool)?;

    Ok(doc_list.add(doc))
}

pub fn confirm_close_all(
    doc_list: &mut SlotList<Doc>,
    reason: &str,
    config: &Config,
    line_pool: &mut LinePool,
    time: f32,
) {
    for doc in doc_list.iter_mut().filter_map(|doc| doc.as_mut()) {
        confirm_close(doc, reason, false, config, line_pool, time);
    }
}
