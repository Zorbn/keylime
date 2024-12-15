#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    cell::{OnceCell, RefCell},
    path::Path,
    rc::Rc,
};

use objc2::{
    declare_class, msg_send, msg_send_id, mutability::MainThreadOnly, rc::Retained,
    runtime::ProtocolObject, ClassType, DeclaredClass,
};
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication,
    NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType, NSColor, NSEvent,
    NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect,
    NSSize,
};
use objc2_metal_kit::{MTKView, MTKViewDelegate};

use crate::{
    app::App,
    input::{
        input_handlers::{CharHandler, KeybindHandler, MouseScrollHandler, MousebindHandler},
        keybind::Keybind,
        mouse_scroll::MouseScroll,
        mousebind::Mousebind,
    },
    temp_buffer::TempBuffer,
};

use super::{file_watcher::FileWatcher, gfx::Gfx, pty::Pty, result::Result};

macro_rules! idcell {
    ($name:ident => $this:expr) => {
        $this.ivars().$name.set($name).expect(&format!(
            "ivar should not be initialized: `{}`",
            stringify!($name)
        ));
    };
    ($name:ident <= $this:expr) => {
        let Some($name) = $this.ivars().$name.get() else {
            unreachable!("ivar should be initialized: `{}`", stringify!($name));
        };
    };
}

struct Ivars {
    ns_window: OnceCell<Retained<NSWindow>>,
    app: Rc<RefCell<App>>,
    window: Rc<RefCell<Window>>,
}

declare_class!(
    struct Delegate;

    unsafe impl ClassType for Delegate {
        type Super = NSObject;
        type Mutability = MainThreadOnly;
        const NAME: &'static str = "Delegate";
    }

    impl DeclaredClass for Delegate {
        type Ivars = Ivars;
    }

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl NSApplicationDelegate for Delegate {
        #[method(applicationDidFinishLaunching:)]
        #[allow(non_snake_case)]
        unsafe fn applicationDidFinishLaunching(&self, _notification: &NSNotification) {
            let window = self.ivars().window.clone();
            let app = self.ivars().app.borrow();

            let mtm = MainThreadMarker::from(self);

            let content_rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(768.0, 768.0));

            let ns_window = {

                let style = NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Resizable
                    | NSWindowStyleMask::Miniaturizable
                    | NSWindowStyleMask::Titled;

                unsafe {
                    NSWindow::initWithContentRect_styleMask_backing_defer(mtm.alloc(), content_rect, style, NSBackingStoreType::NSBackingStoreBuffered, false)
                }
            };

            unsafe {
                let theme = &app.config().theme;

                let r = theme.background.r as f64 / 255.0f64;
                let g = theme.background.g as f64 / 255.0f64;
                let b = theme.background.b as f64 / 255.0f64;
                let a = theme.background.a as f64 / 255.0f64;

                ns_window.setBackgroundColor(Some(&NSColor::colorWithRed_green_blue_alpha(r, g, b, a)));
                ns_window.setAcceptsMouseMovedEvents(true);
            }

            let mut gfx = unsafe {
                let protocol_object = ProtocolObject::from_ref(self);
                Gfx::new("Menlo", 12.0, window.clone(), &ns_window, mtm, protocol_object).unwrap()
            };

            gfx.resize(content_rect.size.width as i32, content_rect.size.height as i32).unwrap();

            ns_window.center();
            ns_window.setTitle(ns_string!("Keylime"));
            ns_window.makeKeyAndOrderFront(None);

            window.borrow_mut().gfx = Some(gfx);

            idcell!(ns_window => self);

            unsafe {
                let app: &mut NSApplication = msg_send![_notification, object];
                app.activate();
            }
        }

        #[method(applicationShouldTerminateAfterLastWindowClosed:)]
        #[allow(non_snake_case)]
        unsafe fn applicationShouldTerminateAfterLastWindowClosed(&self, _sender: &NSApplication) -> bool {
            true
        }
    }

    unsafe impl MTKViewDelegate for Delegate {
        #[method(drawInMTKView:)]
        #[allow(non_snake_case)]
        unsafe fn drawInMTKView(&self, _view: &MTKView) {
            let window = &mut *self.ivars().window.borrow_mut();
            let mut app = self.ivars().app.borrow_mut();

            app.update(window);
            window.clear_inputs();
            app.draw(window);
        }

        #[method(mtkView:drawableSizeWillChange:)]
        #[allow(non_snake_case)]
        unsafe fn mtkView_drawableSizeWillChange(&self, _view: &MTKView, size: NSSize) {
            // TODO: Handle resize.
            let window = &mut *self.ivars().window.borrow_mut();

            if let Some(gfx) = &mut window.gfx {
                gfx.resize(size.width as i32, size.height as i32).unwrap();
            }
        }
    }
);

impl Delegate {
    fn new(app: Rc<RefCell<App>>, mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc();
        let this = this.set_ivars(Ivars {
            ns_window: OnceCell::default(),
            window: Rc::new(RefCell::new(Window::new())),
            app,
        });

        unsafe { msg_send_id![super(this), init] }
    }
}

pub struct WindowRunner {
    app: Rc<RefCell<App>>,
}

impl WindowRunner {
    pub fn new(app: App) -> Result<Box<Self>> {
        Ok(Box::new(WindowRunner {
            app: Rc::new(RefCell::new(app)),
        }))
    }

    pub fn run(&mut self) {
        let mtm = MainThreadMarker::new().unwrap();

        let appearance_name = unsafe {
            if self.app.borrow().is_dark() {
                NSAppearanceNameDarkAqua
            } else {
                NSAppearanceNameAqua
            }
        };

        let appearance = NSAppearance::appearanceNamed(&appearance_name);

        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        app.setAppearance(appearance.as_deref());

        let delegate = Delegate::new(self.app.clone(), mtm);
        let object = ProtocolObject::from_ref(&*delegate);
        app.setDelegate(Some(object));

        unsafe {
            app.run();
        }
    }
}

pub struct Window {
    gfx: Option<Gfx>,
    file_watcher: FileWatcher,

    wide_text_buffer: TempBuffer<u16>,

    pub chars_typed: Vec<char>,
    pub keybinds_typed: Vec<Keybind>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,
}

impl Window {
    pub fn new() -> Self {
        Self {
            gfx: None,
            file_watcher: FileWatcher {},

            wide_text_buffer: TempBuffer::new(),

            chars_typed: Vec::new(),
            keybinds_typed: Vec::new(),
            mousebinds_pressed: Vec::new(),
            mouse_scrolls: Vec::new(),
        }
    }

    pub fn update<'a>(
        &mut self,
        is_animating: bool,
        ptys: impl Iterator<Item = &'a Pty>,
        files: impl Iterator<Item = &'a Path>,
    ) -> (f32, f32) {
        (0.0, 0.0)
    }

    fn clear_inputs(&mut self) {
        self.chars_typed.clear();
    }

    pub fn handle_key_down(&mut self, event: &NSEvent) {
        if let Some(characters) = unsafe { event.characters() } {
            let wide_text_buffer = self.wide_text_buffer.get_mut();

            for i in 0..characters.length() {
                let wide_char = unsafe { characters.characterAtIndex(i) };

                wide_text_buffer.push(wide_char);
            }

            for c in char::decode_utf16(wide_text_buffer.iter().copied()) {
                let Ok(c) = c else {
                    continue;
                };

                if c.is_control() {
                    continue;
                }

                self.chars_typed.push(c);
            }
        }
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
        CharHandler::new(self.chars_typed.len())
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
