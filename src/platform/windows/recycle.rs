use std::{
    path::Path,
    ptr::{null, null_mut},
};

use windows::{
    core::{Error, Result, PCWSTR},
    Win32::{
        Foundation::{E_FAIL, FALSE, HWND},
        UI::Shell::{SHFileOperationW, FOF_ALLOWUNDO, FOF_NO_UI, FO_DELETE, SHFILEOPSTRUCTW},
    },
};

pub fn recycle(path: &Path) -> Result<()> {
    let Some(path_str) = path.as_os_str().to_str() else {
        return Err(Error::new(E_FAIL, "Invalid path"));
    };

    let mut wide_path = Vec::new();

    for c in path_str.chars() {
        let mut dst = [0u16; 2];

        for wide_c in c.encode_utf16(&mut dst) {
            wide_path.push(*wide_c);
        }
    }

    // SHFILEOPSTRUCTW requires paths be double null terminated.
    wide_path.extend_from_slice(&[0, 0]);

    let mut file_op = SHFILEOPSTRUCTW {
        hwnd: HWND(null_mut()),
        wFunc: FO_DELETE,
        pFrom: PCWSTR(wide_path.as_ptr()),
        pTo: PCWSTR(null()),
        fFlags: (FOF_ALLOWUNDO | FOF_NO_UI).0 as u16,
        fAnyOperationsAborted: FALSE,
        hNameMappings: null_mut(),
        lpszProgressTitle: PCWSTR(null()),
    };

    let result = unsafe { SHFileOperationW(&mut file_op) };

    if result == 0 {
        Ok(())
    } else {
        Err(Error::new(E_FAIL, "Failed to recycle file"))
    }
}
