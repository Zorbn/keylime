use std::{
    io,
    path::{absolute, Path},
};

use crate::{
    ctx::Ctx,
    platform::dialog::{find_file, message, FindFileKind, MessageKind, MessageResponse},
    text::doc::{Doc, DocKind},
    ui::slot_list::SlotList,
};

pub fn confirm_close(doc: &mut Doc, reason: &str, is_cancelable: bool, ctx: &mut Ctx) -> bool {
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
            MessageResponse::Yes => try_save(doc, ctx),
            MessageResponse::No => true,
            MessageResponse::Cancel => false,
        }
    }
}

pub fn try_save(doc: &mut Doc, ctx: &mut Ctx) -> bool {
    let path = if doc.path().is_none() {
        let Ok(path) = find_file(FindFileKind::Save) else {
            return false;
        };

        Some(path)
    } else {
        None
    };

    if ctx.config.trim_trailing_whitespace {
        doc.trim_trailing_whitespace(ctx);
    }

    if let Err(err) = doc.save(path) {
        message("Failed to Save File", &err.to_string(), MessageKind::Ok);
        false
    } else {
        true
    }
}

pub fn open_or_reuse(
    doc_list: &mut SlotList<Doc>,
    path: &Path,
    ctx: &mut Ctx,
) -> io::Result<usize> {
    let path = absolute(path)?;

    for (i, doc) in doc_list.iter().enumerate() {
        if doc.as_ref().and_then(|doc| doc.path().some()) == Some(&path) {
            return Ok(i);
        }
    }

    let mut doc = Doc::new(Some(path), &mut ctx.buffers.lines, None, DocKind::MultiLine);

    doc.load(ctx)?;

    Ok(doc_list.add(doc))
}

pub fn confirm_close_all(doc_list: &mut SlotList<Doc>, reason: &str, ctx: &mut Ctx) {
    for doc in doc_list.iter_mut().filter_map(|doc| doc.as_mut()) {
        confirm_close(doc, reason, false, ctx);
    }
}
