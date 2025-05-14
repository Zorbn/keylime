#![deny(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;

use objc2::{
    define_class, msg_send, rc::Retained, runtime::ProtocolObject, DefinedClass, MainThreadMarker,
    MainThreadOnly,
};
use objc2_app_kit::{NSApplication, NSApplicationDelegate, NSWindow, NSWindowDelegate};
use objc2_foundation::{ns_string, NSNotification, NSObject, NSObjectProtocol, NSThread};
use objc2_metal::MTLCreateSystemDefaultDevice;

use crate::{
    app::App,
    platform::{
        aliases::{AnyGfx, AnyWindow},
        platform_impl::view::View,
    },
};

use super::{gfx::Gfx, window::Window};

pub struct AppDelegateIvars {
    window_delegates: RefCell<Vec<Retained<WindowDelegate>>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AppDelegate"]
    #[ivars = AppDelegateIvars]
    pub struct AppDelegate;

    impl AppDelegate {
        #[unsafe(method(newWindow))]
        fn new_window(&self) {
            let mut window_delegates = self.ivars().window_delegates.borrow_mut();

            let mtm = MainThreadMarker::from(self);

            window_delegates.push(WindowDelegate::new(mtm));
        }

        #[unsafe(method(onWindowShouldClose))]
        fn on_window_should_close(&self) {
            let mut window_delegates = self.ivars().window_delegates.borrow_mut();

            window_delegates.retain(|window_delegate| {
                *window_delegate.ivars().is_running.borrow()
            });
        }
    }

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        unsafe fn application_did_finish_launching(&self, notification: &NSNotification) {
            let mut window_delegates = self.ivars().window_delegates.borrow_mut();

            let mtm = MainThreadMarker::from(self);

            window_delegates.push(WindowDelegate::new(mtm));

            // Create a blank thread to tell Cocoa that we are using multi-threading (for the pty).
            // https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/CreatingThreads/CreatingThreads.html
            let _ = NSThread::new();
            assert!(NSThread::isMultiThreaded());

            unsafe {
                let app: &mut NSApplication = msg_send![notification, object];
                app.activate();

                for window_delegate in window_delegates.iter() {
                    let view = &window_delegate.ivars().view;
                    view.update();
                }
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
            let window_delegates = self.ivars().window_delegates.borrow();

            for window_delegate in window_delegates.iter() {
                window_delegate.ivars().view.on_close();
            }

            true
        }
    }
);

impl AppDelegate {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc();
        let this = this.set_ivars(AppDelegateIvars {
            window_delegates: RefCell::new(Vec::new()),
        });

        unsafe { msg_send![super(this), init] }
    }
}

pub struct WindowDelegateIvars {
    is_running: RefCell<bool>,
    view: Retained<View>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "WindowDelegate"]
    #[ivars = WindowDelegateIvars]
    pub struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowShouldClose:))]
        unsafe fn window_should_close(&self, _sender: &NSWindow) -> bool {
            if let Ok(mut is_running) = self.ivars().is_running.try_borrow_mut() {
                if !*is_running {
                    return true.into();
                }

                *is_running = false;
            }

            self.ivars().view.on_close();

            let mtm = MainThreadMarker::from(self);
            let app = NSApplication::sharedApplication(mtm);

            unsafe {
                if let Some(app_delegate) = app.delegate() {
                    let _: () = msg_send![&app_delegate, onWindowShouldClose];
                }
            }

            true
        }

        #[unsafe(method(windowDidBecomeKey:))]
        unsafe fn window_did_become_key(&self, _notification: &NSNotification) {
            self.ivars().view.on_focused_changed(true);
        }

        #[unsafe(method(windowDidResignKey:))]
        unsafe fn window_did_resign_key(&self, _notification: &NSNotification) {
            self.ivars().view.on_focused_changed(false);
        }

        #[unsafe(method(windowWillEnterFullScreen:))]
        unsafe fn window_will_enter_fullscreen(&self, _notification: &NSNotification) {
            self.ivars().view.on_fullscreen_changed(true);
        }

        #[unsafe(method(windowWillExitFullScreen:))]
        unsafe fn window_will_exit_fullscreen(&self, _notification: &NSNotification) {
            self.ivars().view.on_fullscreen_changed(false);
        }
    }
);

impl WindowDelegate {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let mut window = AnyWindow {
            inner: Window::new(mtm),
        };

        let device = MTLCreateSystemDefaultDevice().expect("Failed to get default system device.");

        let mut gfx = AnyGfx {
            inner: {
                let mut gfx = Gfx::new(&window, device.clone()).unwrap();
                gfx.resize(window.inner.width, window.inner.height).unwrap();

                gfx
            },
        };

        let app = App::new(&mut window, &mut gfx, 0.0);

        let ns_window = window.inner.ns_window.clone();
        ns_window.setAcceptsMouseMovedEvents(true);

        let view = View::new(app, window, gfx, mtm, ns_window.frame(), device);
        ns_window.setContentView(Some(&view));

        ns_window.center();
        ns_window.setTitle(ns_string!("Keylime"));

        let this = mtm.alloc();
        let this = this.set_ivars(WindowDelegateIvars {
            is_running: RefCell::new(true),
            view,
        });

        let this: Retained<Self> = unsafe { msg_send![super(this), init] };

        let protocol_object = ProtocolObject::from_retained(this.clone());
        ns_window.setDelegate(Some(&protocol_object));

        this
    }
}
