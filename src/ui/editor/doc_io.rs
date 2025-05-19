use std::{io, path::Path};

use crate::{
    ctx::Ctx,
    normalizable::Normalizable,
    platform::dialog::{find_file, message, FindFileKind, MessageKind, MessageResponse},
    pool::format_pooled,
    text::doc::{Doc, DocFlags},
    ui::slot_list::{SlotId, SlotList},
};

pub fn confirm_close(doc: &mut Doc, reason: &str, is_cancelable: bool, ctx: &mut Ctx) -> bool {
    if doc.is_saved() {
        true
    } else {
        let text = format_pooled!(
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

    if ctx.config.format_on_save {
        doc.lsp_formatting(ctx);
    }

    if let Err(err) = doc.save(path, ctx) {
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
) -> io::Result<SlotId> {
    let path = path.normalized()?;

    for (id, doc) in doc_list.enumerate() {
        if doc.path().some() == Some(&path) {
            return Ok(id);
        }
    }

    let mut doc = Doc::new(Some(path), None, DocFlags::MULTI_LINE);
    doc.load(ctx)?;

    Ok(doc_list.add(doc))
}

pub fn confirm_close_all(doc_list: &mut SlotList<Doc>, reason: &str, ctx: &mut Ctx) {
    for doc in doc_list.iter_mut() {
        confirm_close(doc, reason, false, ctx);
    }
}
