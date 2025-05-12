#![deny(unsafe_op_in_unsafe_fn)]

use std::{cell::RefCell, rc::Rc};

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
    window_delegates: Rc<RefCell<Vec<Retained<WindowDelegate>>>>,
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
                let window = window_delegate.ivars().window.borrow();
                window.inner.is_running
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
                    let view = {
                        window_delegate.ivars().window.borrow_mut().inner.view.clone()
                    };

                    if let Some(view) = view {
                        view.update();
                    }
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
                window_delegate.on_close();
            }

            true
        }
    }
);

impl AppDelegate {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc();
        let this = this.set_ivars(AppDelegateIvars {
            window_delegates: Rc::new(RefCell::new(Vec::new())),
        });

        unsafe { msg_send![super(this), init] }
    }
}

pub struct WindowDelegateIvars {
    app: Rc<RefCell<Option<App>>>,
    window: Rc<RefCell<AnyWindow>>,
    gfx: Rc<RefCell<Option<AnyGfx>>>,
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
            self.on_close();

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
            self.on_focused_changed(true);
        }

        #[unsafe(method(windowDidResignKey:))]
        unsafe fn window_did_resign_key(&self, _notification: &NSNotification) {
            self.on_focused_changed(false);
        }

        #[unsafe(method(windowWillEnterFullScreen:))]
        unsafe fn window_will_enter_fullscreen(&self, _notification: &NSNotification) {
            self.on_fullscreen_changed(true);
        }

        #[unsafe(method(windowWillExitFullScreen:))]
        unsafe fn window_will_exit_fullscreen(&self, _notification: &NSNotification) {
            self.on_fullscreen_changed(false);
        }
    }
);

impl WindowDelegate {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let app = App::new();

        let window = AnyWindow {
            inner: Window::new(&app, mtm),
        };

        let window = Rc::new(RefCell::new(window));
        let app = Rc::new(RefCell::new(Some(app)));
        let gfx = Rc::new(RefCell::new(None));

        let (ns_window, width, height) = {
            let window = window.borrow();

            (
                window.inner.ns_window.clone(),
                window.inner.width,
                window.inner.height,
            )
        };

        ns_window.setAcceptsMouseMovedEvents(true);

        let device = MTLCreateSystemDefaultDevice().expect("Failed to get default system device.");

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
                let app = app.borrow();
                let app = app.as_ref().unwrap();
                let window = &*window.borrow();

                let mut gfx = Gfx::new(app, window, device, view.clone()).unwrap();
                gfx.resize(width, height).unwrap();

                gfx
            },
        }));

        ns_window.setContentView(Some(&view));
        ns_window.center();
        ns_window.setTitle(ns_string!("Keylime"));

        let this = mtm.alloc();
        let this = this.set_ivars(WindowDelegateIvars { window, gfx, app });

        let this: Retained<Self> = unsafe { msg_send![super(this), init] };

        let protocol_object = ProtocolObject::from_retained(this.clone());
        ns_window.setDelegate(Some(&protocol_object));

        this
    }

    fn on_focused_changed(&self, is_focused: bool) -> Option<()> {
        let mut window = self.ivars().window.try_borrow_mut().ok()?;

        let mut gfx = self.ivars().gfx.try_borrow_mut().ok()?;
        let gfx = gfx.as_mut()?;

        window.inner.is_focused = is_focused;

        let view = gfx.inner.view();

        unsafe {
            view.setNeedsDisplay(true);
        }

        Some(())
    }

    fn on_fullscreen_changed(&self, is_fullscreen: bool) -> Option<()> {
        let mut gfx = self.ivars().gfx.try_borrow_mut().ok()?;
        let gfx = gfx.as_mut()?;

        gfx.inner.is_fullscreen = is_fullscreen;

        Some(())
    }

    fn on_close(&self) {
        let window = &mut *self.ivars().window.borrow_mut();
        let mut gfx = self.ivars().gfx.borrow_mut();
        let gfx = gfx.as_mut().unwrap();

        if !window.inner.is_running {
            return;
        }

        window.inner.is_running = false;

        let mut app = self.ivars().app.borrow_mut();

        {
            let app = app.as_mut().unwrap();
            let time = window.inner.time;

            app.close(window, gfx, time);
        }

        app.take();
    }
}
