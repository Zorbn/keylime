use std::path::PathBuf;

use crate::platform::dialog::{FindFileKind, MessageKind, MessageResponse};

use super::result::Result;

pub fn find_file(_kind: FindFileKind) -> Result<PathBuf> {
    Err("Unavailable while testing")
}

pub fn message(_title: &str, _text: &str, _kind: MessageKind) -> MessageResponse {
    MessageResponse::Cancel
}
