#![deny(unsafe_op_in_unsafe_fn)]

use std::{cell::RefCell, rc::Rc};

use objc2::{
    define_class, msg_send, rc::Retained, runtime::ProtocolObject, DefinedClass, MainThreadMarker,
    MainThreadOnly,
};
use objc2_app_kit::{
    NSApplication, NSApplicationDelegate, NSColor, NSMenuItem, NSWindow, NSWindowDelegate,
};
use objc2_foundation::{ns_string, NSNotification, NSObject, NSObjectProtocol, NSThread};

use crate::{
    app::App,
    platform::{
        aliases::{AnyGfx, AnyWindow},
        platform_impl::window::{ENTER_FULL_SCREEN_TITLE, EXIT_FULL_SCREEN_TITLE},
    },
};

use super::{gfx::Gfx, window::Window};

pub struct DelegateIvars {
    app: Rc<RefCell<App>>,
    window: Rc<RefCell<AnyWindow>>,
    fullscreen_item: Retained<NSMenuItem>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "Delegate"]
    #[ivars = DelegateIvars]
    pub struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl NSApplicationDelegate for Delegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        unsafe fn application_did_finish_launching(&self, notification: &NSNotification) {
            let window = self.ivars().window.clone();
            let app = self.ivars().app.clone();

            let mtm = MainThreadMarker::from(self);

            let (ns_window, width, height) = {
                let window = window.borrow();

                (
                    window.inner.ns_window.clone(),
                    window.inner.width,
                    window.inner.height,
                )
            };

            unsafe {
                let app = app.borrow();

                let theme = &app.config().theme;

                let r = theme.background.r as f64 / 255.0f64;
                let g = theme.background.g as f64 / 255.0f64;
                let b = theme.background.b as f64 / 255.0f64;
                let a = theme.background.a as f64 / 255.0f64;

                ns_window
                    .setBackgroundColor(Some(&NSColor::colorWithRed_green_blue_alpha(r, g, b, a)));
                ns_window.setAcceptsMouseMovedEvents(true);
            }

            let protocol_object = ProtocolObject::from_ref(self);
            ns_window.setDelegate(Some(protocol_object));

            let mut gfx = Gfx::new(app.clone(), window.clone(), &ns_window, mtm).unwrap();

            gfx.resize(width, height).unwrap();

            let view = gfx.view().clone();

            window.borrow_mut().inner.gfx = Some(AnyGfx { inner: gfx });

            ns_window.setContentView(Some(&view));
            ns_window.center();
            ns_window.setTitle(ns_string!("Keylime"));

            // Create a blank thread to tell Cocoa that we are using multi-threading (for the pty).
            // https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/CreatingThreads/CreatingThreads.html
            let _ = NSThread::new();
            assert!(NSThread::isMultiThreaded());

            unsafe {
                let app: &mut NSApplication = msg_send![notification, object];
                app.activate();
                view.update();
            }
        }

        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        unsafe fn application_should_terminate_after_last_window_closed(
            &self,
            _sender: &NSApplication,
        ) -> bool {
            true
        }

        #[unsafe(method(applicationShouldTerminate:))]
        unsafe fn application_should_terminate(&self, _sender: &NSApplication) -> bool {
            self.on_close();

            true
        }
    }

    unsafe impl NSWindowDelegate for Delegate {
        #[unsafe(method(windowShouldClose:))]
        unsafe fn window_should_close(&self, _sender: &NSWindow) -> bool {
            self.on_close();

            true
        }

        #[unsafe(method(windowDidBecomeKey:))]
        unsafe fn window_did_become_key(&self, _notification: &NSNotification) {
            self.on_focused_changed(true);
        }

        #[unsafe(method(windowDidResignKey:))]
        unsafe fn window_did_resign_key(&self, _notification: &NSNotification) {
            self.on_focused_changed(false);
        }

        #[unsafe(method(windowWillEnterFullScreen:))]
        unsafe fn window_will_enter_fullscreen(&self, _notification: &NSNotification) {
            let fullscreen_item = &self.ivars().fullscreen_item;

            unsafe {
                fullscreen_item.setTitle(ns_string!(EXIT_FULL_SCREEN_TITLE));
            }

            self.on_fullscreen_changed(true);
        }

        #[unsafe(method(windowWillExitFullScreen:))]
        unsafe fn window_will_exit_fullscreen(&self, _notification: &NSNotification) {
            let fullscreen_item = &self.ivars().fullscreen_item;

            unsafe {
                fullscreen_item.setTitle(ns_string!(ENTER_FULL_SCREEN_TITLE));
            }

            self.on_fullscreen_changed(false);
        }
    }
);

impl Delegate {
    pub fn new(
        app: Rc<RefCell<App>>,
        fullscreen_item: Retained<NSMenuItem>,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let window = AnyWindow {
            inner: Window::new(mtm),
        };

        let this = mtm.alloc();
        let this = this.set_ivars(DelegateIvars {
            window: Rc::new(RefCell::new(window)),
            app,
            fullscreen_item,
        });

        unsafe { msg_send![super(this), init] }
    }

    fn on_focused_changed(&self, is_focused: bool) {
        let Ok(mut window) = self.ivars().window.try_borrow_mut() else {
            return;
        };

        window.inner.is_focused = is_focused;

        let view = window.gfx().inner.view();

        unsafe {
            view.setNeedsDisplay(true);
        }
    }

    fn on_fullscreen_changed(&self, is_fullscreen: bool) {
        let Ok(mut window) = self.ivars().window.try_borrow_mut() else {
            return;
        };

        window.gfx().inner.is_fullscreen = is_fullscreen;
    }

    fn on_close(&self) {
        let mut window = self.ivars().window.borrow_mut();

        if !window.inner.is_running {
            return;
        }

        window.inner.is_running = false;

        let mut app = self.ivars().app.borrow_mut();

        let time = window.inner.time;

        app.close(time);
    }
}
