use std::path::PathBuf;

use objc2::rc::Retained;
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertStyle, NSBackingStoreType,
    NSOpenPanel, NSSavePanel, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, CGPoint, CGRect, CGSize, MainThreadMarker, NSRect, NSString, NSURL,
};

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
    let mtm = MainThreadMarker::new().unwrap();

    let content_rect = CGRect::new(CGPoint::ZERO, CGSize::new(500.0, 500.0));
    let style = NSWindowStyleMask::UtilityWindow;

    unsafe {
        let url = if kind == FindFileKind::Save {
            find_file_save(mtm, content_rect, style)
        } else {
            find_file_open(kind, mtm, content_rect, style)
        };

        let path = url
            .ok_or("Dialog is missing a URL")?
            .path()
            .ok_or("URL doesn't correspond to a path")?;

        Ok(PathBuf::from(path.to_string()))
    }
}

unsafe fn find_file_open(
    kind: FindFileKind,
    mtm: MainThreadMarker,
    content_rect: NSRect,
    style: NSWindowStyleMask,
) -> Option<Retained<NSURL>> {
    let open_panel = NSOpenPanel::initWithContentRect_styleMask_backing_defer(
        mtm.alloc(),
        content_rect,
        style,
        NSBackingStoreType::NSBackingStoreBuffered,
        true,
    );

    match kind {
        FindFileKind::OpenFile => open_panel.setCanChooseFiles(true),
        FindFileKind::OpenFolder => open_panel.setCanChooseDirectories(true),
        _ => {}
    }

    open_panel.runModal();
    open_panel.URL()
}

unsafe fn find_file_save(
    mtm: MainThreadMarker,
    content_rect: NSRect,
    style: NSWindowStyleMask,
) -> Option<Retained<NSURL>> {
    let save_panel = NSSavePanel::initWithContentRect_styleMask_backing_defer(
        mtm.alloc(),
        content_rect,
        style,
        NSBackingStoreType::NSBackingStoreBuffered,
        true,
    );

    save_panel.runModal();
    save_panel.URL()
}

pub fn message(title: &str, text: &str, kind: MessageKind) -> MessageResponse {
    let mtm = MainThreadMarker::new().unwrap();

    let response = unsafe {
        let alert = NSAlert::init(mtm.alloc());
        alert.setMessageText(&NSString::from_str(title));
        alert.setInformativeText(&NSString::from_str(text));
        alert.setAlertStyle(NSAlertStyle::Warning);

        if matches!(kind, MessageKind::YesNo | MessageKind::YesNoCancel) {
            alert.addButtonWithTitle(ns_string!("Save"));
            alert.addButtonWithTitle(ns_string!("Don't Save"));
        }

        if kind == MessageKind::YesNoCancel {
            alert.addButtonWithTitle(ns_string!("Cancel"));
        }

        alert.runModal()
    };

    if response == NSAlertFirstButtonReturn {
        MessageResponse::Yes
    } else if response == NSAlertSecondButtonReturn {
        MessageResponse::No
    } else {
        MessageResponse::Cancel
    }
}
