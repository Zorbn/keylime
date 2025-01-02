use std::path::Path;

use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSArray, NSString, NSURL};

use super::result::Result;

pub fn recycle(path: &Path) -> Result<()> {
    let Some(path_str) = path.as_os_str().to_str() else {
        return Err("Invalid path");
    };

    let path_str = NSString::from_str(path_str);

    unsafe {
        let urls = NSArray::from_retained_slice(&[NSURL::fileURLWithPath(&path_str)]);
        NSWorkspace::sharedWorkspace().recycleURLs_completionHandler(&urls, None);
    }

    Ok(())
}
