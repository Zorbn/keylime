use std::path::Path;

use cocoa::{
    appkit::{
        NSApp, NSApplication,
        NSApplicationActivationOptions::NSApplicationActivateIgnoringOtherApps,
        NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular,
        NSBackingStoreType::NSBackingStoreBuffered, NSEvent, NSEventMask, NSEventSubtype,
        NSEventType, NSMenu, NSMenuItem, NSPasteboard, NSRunningApplication, NSWindow,
        NSWindowStyleMask,
    },
    base::{id, nil, selector, NO, YES},
    foundation::{
        NSAutoreleasePool, NSDate, NSDefaultRunLoopMode, NSPoint, NSProcessInfo, NSRect, NSSize,
        NSString,
    },
};
use metal::objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Object, Sel},
    sel, sel_impl,
};

use crate::{
    app::App,
    input::{
        input_handlers::{CharHandler, KeybindHandler, MouseScrollHandler, MousebindHandler},
        keybind::Keybind,
        mouse_scroll::MouseScroll,
        mousebind::Mousebind,
    },
};

use super::{file_watcher::FileWatcher, gfx::Gfx, pty::Pty, result::Result};

pub struct WindowRunner {
    app: App,
}

impl WindowRunner {
    pub fn new(app: App) -> Result<Box<Self>> {
        Ok(Box::new(WindowRunner { app }))
    }

    pub fn run(&mut self) {
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);

            let superclass = class!(NSObject);
            let mut app_delegate_decl = ClassDecl::new("KeylimeAppDelegate", superclass).unwrap();

            extern "C" fn application_should_terminate_after_last_window_closed(
                _: &Object,
                _: Sel,
                _: id,
            ) -> bool {
                true
            }

            app_delegate_decl.add_method(
                sel!(applicationShouldTerminateAfterLastWindowClosed:),
                application_should_terminate_after_last_window_closed
                    as extern "C" fn(&Object, Sel, id) -> bool,
            );

            let app_delegate_class = app_delegate_decl.register();
            let app_delegate_object = msg_send![app_delegate_class, new];

            let app = NSApp();
            app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
            app.setDelegate_(app_delegate_object);

            let menubar = NSMenu::new(nil).autorelease();
            let app_menu_item = NSMenuItem::new(nil).autorelease();
            menubar.addItem_(app_menu_item);
            app.setMainMenu_(menubar);

            let app_menu = NSMenu::new(nil).autorelease();
            let quit_prefix = NSString::alloc(nil).init_str("Quit ");
            let quit_title =
                quit_prefix.stringByAppendingString_(NSProcessInfo::processInfo(nil).processName());
            let quit_action = selector("terminate:");
            let quit_key = NSString::alloc(nil).init_str("q");
            let quit_item = NSMenuItem::alloc(nil)
                .initWithTitle_action_keyEquivalent_(quit_title, quit_action, quit_key)
                .autorelease();
            app_menu.addItem_(quit_item);
            app_menu_item.setSubmenu_(app_menu);

            let mut window_delegate_decl =
                ClassDecl::new("KeylimeWindowDelegate", superclass).unwrap();

            extern "C" fn window_should_close(_: &Object, _: Sel, _: id) -> bool {
                println!("window should close");
                true
            }

            window_delegate_decl.add_method(
                sel!(windowShouldClose:),
                window_should_close as extern "C" fn(&Object, Sel, id) -> bool,
            );

            extern "C" fn window_did_resize(_: &Object, _: Sel, _: id) {
                println!("window did resize");
            }

            window_delegate_decl.add_method(
                sel!(windowDidResize:),
                window_did_resize as extern "C" fn(&Object, Sel, id),
            );

            let window_delegate_class = window_delegate_decl.register();
            let window_delegate_object = msg_send![window_delegate_class, new];

            let window = NSWindow::alloc(nil)
                .initWithContentRect_styleMask_backing_defer_(
                    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(200.0, 200.0)),
                    NSWindowStyleMask::NSTitledWindowMask
                        | NSWindowStyleMask::NSResizableWindowMask
                        | NSWindowStyleMask::NSClosableWindowMask
                        | NSWindowStyleMask::NSMiniaturizableWindowMask,
                    NSBackingStoreBuffered,
                    NO,
                )
                .autorelease();

            window.setDelegate_(window_delegate_object);

            window.cascadeTopLeftFromPoint_(NSPoint::new(20.0, 20.0));
            window.center();

            let title = NSString::alloc(nil).init_str("Keylime");
            window.setTitle_(title);
            window.makeKeyAndOrderFront_(nil);

            let current_app = NSRunningApplication::currentApplication(nil);
            current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);

            app.run();
        }
    }
}

pub struct Window {
    gfx: Option<Gfx>,
    file_watcher: FileWatcher,

    pub chars_typed: Vec<char>,
    pub keybinds_typed: Vec<Keybind>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,
}

impl Window {
    pub fn update<'a>(
        &mut self,
        is_animating: bool,
        ptys: impl Iterator<Item = &'a Pty>,
        files: impl Iterator<Item = &'a Path>,
    ) -> (f32, f32) {
        (0.0, 0.0)
    }

    pub fn is_running(&self) -> bool {
        true
    }

    pub fn is_focused(&self) -> bool {
        true
    }

    pub fn dpi(&self) -> f32 {
        1.0
    }

    pub fn gfx(&mut self) -> &mut Gfx {
        self.gfx.as_mut().unwrap()
    }

    pub fn file_watcher(&self) -> &FileWatcher {
        &self.file_watcher
    }

    pub fn get_char_handler(&self) -> CharHandler {
        CharHandler::new(0)
    }

    pub fn get_keybind_handler(&self) -> KeybindHandler {
        KeybindHandler::new(0)
    }

    pub fn get_mousebind_handler(&self) -> MousebindHandler {
        MousebindHandler::new(0)
    }

    pub fn get_mouse_scroll_handler(&self) -> MouseScrollHandler {
        MouseScrollHandler::new(0)
    }

    pub fn set_clipboard(&mut self, text: &[char], was_copy_implicit: bool) -> Result<()> {
        Ok(())
    }

    pub fn get_clipboard(&mut self) -> Result<&[char]> {
        Ok(&[])
    }

    pub fn was_copy_implicit(&self) -> bool {
        false
    }
}