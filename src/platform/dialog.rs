use std::path::PathBuf;

use crate::pool::Pooled;

use super::{platform_impl, result::Result};

#[derive(PartialEq, Eq, Debug)]
pub enum FindFileKind {
    OpenFile,
    OpenFolder,
    Save,
}

#[derive(PartialEq, Eq, Debug)]
pub enum MessageKind {
    Ok,
    YesNo,
    YesNoCancel,
}

#[derive(PartialEq, Eq, Debug)]
pub enum MessageResponse {
    Yes,
    No,
    Cancel,
}

pub fn find_file(kind: FindFileKind) -> Result<Pooled<PathBuf>> {
    platform_impl::dialog::find_file(kind)
}

pub fn message(title: &str, text: &str, kind: MessageKind) -> MessageResponse {
    platform_impl::dialog::message(title, text, kind)
}
