use std::path::PathBuf;

use super::result::Result;

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

pub fn find_file(kind: FindFileKind) -> Result<PathBuf> {
    Ok(PathBuf::new())
}

pub fn message(title: &str, text: &str, kind: MessageKind) -> MessageResponse {
    MessageResponse::Yes
}
