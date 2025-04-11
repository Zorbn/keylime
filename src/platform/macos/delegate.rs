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
use objc2_metal::MTLCreateSystemDefaultDevice;

use crate::{
    app::App,
    platform::{
        aliases::{AnyGfx, AnyWindow},
        platform_impl::{
            view::View,
            window::{ENTER_FULL_SCREEN_TITLE, EXIT_FULL_SCREEN_TITLE},
        },
    },
};

use super::{gfx::Gfx, window::Window};

pub struct DelegateIvars {
    app: Rc<RefCell<App>>,
    window: Rc<RefCell<AnyWindow>>,
    gfx: Rc<RefCell<Option<AnyGfx>>>,
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
            let gfx = self.ivars().gfx.clone();

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

            let device =
                MTLCreateSystemDefaultDevice().expect("Failed to get default system device.");

            let view = View::new(
                app.clone(),
                window.clone(),
                gfx.clone(),
                mtm,
                ns_window.frame(),
                device.clone(),
            );

            window.borrow_mut().inner.view = Some(view.clone());

            gfx.replace(Some(AnyGfx {
                inner: {
                    let mut gfx =
                        Gfx::new(app.clone(), window.clone(), device, view.clone()).unwrap();
                    gfx.resize(width, height).unwrap();

                    gfx
                },
            }));

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
            gfx: Rc::new(RefCell::new(None)),
            app,
            fullscreen_item,
        });

        unsafe { msg_send![super(this), init] }
    }

    fn on_focused_changed(&self, is_focused: bool) {
        let Ok(mut window) = self.ivars().window.try_borrow_mut() else {
            return;
        };

        let Ok(mut gfx) = self.ivars().gfx.try_borrow_mut() else {
            return;
        };

        let Some(gfx) = gfx.as_mut() else {
            return;
        };

        window.inner.is_focused = is_focused;

        let view = gfx.inner.view();

        unsafe {
            view.setNeedsDisplay(true);
        }
    }

    fn on_fullscreen_changed(&self, is_fullscreen: bool) {
        let Ok(mut gfx) = self.ivars().gfx.try_borrow_mut() else {
            return;
        };

        let Some(gfx) = gfx.as_mut() else {
            return;
        };

        gfx.inner.is_fullscreen = is_fullscreen;
    }

    fn on_close(&self) {
        let mut window = self.ivars().window.borrow_mut();
        let mut gfx = self.ivars().gfx.borrow_mut();
        let gfx = gfx.as_mut().unwrap();

        if !window.inner.is_running {
            return;
        }

        window.inner.is_running = false;

        let mut app = self.ivars().app.borrow_mut();

        let time = window.inner.time;

        app.close(gfx, time);
    }
}
