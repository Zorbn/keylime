use std::path::PathBuf;

use windows::{
    core::{Result, HSTRING},
    Win32::{
        System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER},
        UI::{
            Shell::{
                FileOpenDialog, FileSaveDialog, IFileDialog, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
            },
            WindowsAndMessaging::{
                MessageBoxW, IDNO, IDYES, MB_ICONWARNING, MB_OK, MB_YESNO, MB_YESNOCANCEL,
            },
        },
    },
};

use crate::{
    platform::dialog::{FindFileKind, MessageKind, MessageResponse},
    pool::{Pooled, PATH_POOL},
};

use super::deferred_call::defer;

pub fn find_file(kind: FindFileKind) -> Result<Pooled<PathBuf>> {
    let dialog_id = match kind {
        FindFileKind::OpenFile | FindFileKind::OpenFolder => FileOpenDialog,
        FindFileKind::Save => FileSaveDialog,
    };

    unsafe {
        let dialog: IFileDialog = CoCreateInstance(&dialog_id, None, CLSCTX_INPROC_SERVER)?;

        if kind == FindFileKind::OpenFolder {
            dialog.SetOptions(FOS_PICKFOLDERS)?;
        }

        dialog.Show(None)?;

        let result = dialog.GetResult()?;
        let wide_path = result.GetDisplayName(SIGDN_FILESYSPATH)?;

        defer!({ CoTaskMemFree(Some(wide_path.0 as _)) });

        let wide_path = wide_path.to_string()?;

        Ok(PATH_POOL.init_item(|path| path.push(&wide_path)))
    }
}

pub fn message(title: &str, text: &str, kind: MessageKind) -> MessageResponse {
    let style = match kind {
        MessageKind::Ok => MB_OK,
        MessageKind::YesNo => MB_YESNO,
        MessageKind::YesNoCancel => MB_YESNOCANCEL,
    } | MB_ICONWARNING;

    unsafe {
        let wide_title = HSTRING::from(title);
        let wide_text = HSTRING::from(text);

        match MessageBoxW(None, &wide_text, &wide_title, style) {
            IDYES => MessageResponse::Yes,
            IDNO => MessageResponse::No,
            _ => MessageResponse::Cancel,
        }
    }
}
