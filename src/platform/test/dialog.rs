use std::path::PathBuf;

use crate::{
    platform::dialog::{FindFileKind, MessageKind, MessageResponse},
    pool::Pooled,
};

use super::result::Result;

pub fn find_file(_kind: FindFileKind) -> Result<Pooled<PathBuf>> {
    Err("Unavailable while testing")
}

pub fn message(_title: &str, _text: &str, _kind: MessageKind) -> MessageResponse {
    MessageResponse::Cancel
}
