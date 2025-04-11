#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    cell::{OnceCell, RefCell},
    rc::Rc,
};

use objc2::{
    define_class, msg_send, rc::Retained, runtime::ProtocolObject, sel, DefinedClass,
    MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSEvent, NSView, NSViewLayerContentsPlacement, NSViewLayerContentsRedrawPolicy,
};
use objc2_core_foundation::CGSize;
use objc2_foundation::{NSObjectNSThreadPerformAdditions, NSObjectProtocol, NSRect, NSSize};
use objc2_metal::MTLDevice;
use objc2_quartz_core::{
    CAAutoresizingMask, CALayer, CALayerDelegate, CAMetalDrawable, CAMetalLayer,
};

use crate::{
    app::App,
    config::Config,
    platform::aliases::{AnyGfx, AnyWindow},
};

use super::gfx::PIXEL_FORMAT;

macro_rules! handle_event {
    ($handler:ident, $self:expr, $event:expr $(, $args:expr)*) => {
        if let Ok(mut window) = $self.ivars().window.try_borrow_mut() {
            window.inner.$handler($event, $($args), *);
        }

        $self.update();
    };
}

pub struct ViewIvars {
    app: Rc<RefCell<App>>,
    window: Rc<RefCell<AnyWindow>>,
    gfx: Rc<RefCell<Option<AnyGfx>>>,
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    metal_layer: OnceCell<Retained<CAMetalLayer>>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "View"]
    #[ivars = ViewIvars]
    pub struct View;

    impl View {
        #[unsafe(method_id(makeBackingLayer))]
        unsafe fn make_backing_layer(&self) -> Retained<CALayer> {
            let metal_layer = unsafe { CAMetalLayer::new() };

            unsafe {
                metal_layer.setPixelFormat(PIXEL_FORMAT);
                metal_layer.setDevice(Some(&self.ivars().device));

                let protocol_object = ProtocolObject::from_ref(self);
                metal_layer.setDelegate(Some(protocol_object));

                metal_layer.setAllowsNextDrawableTimeout(false);

                metal_layer.setAutoresizingMask(
                    CAAutoresizingMask::LayerWidthSizable
                        | CAAutoresizingMask::LayerHeightSizable,
                );

                metal_layer.setNeedsDisplayOnBoundsChange(true);
            }

            self.ivars().metal_layer.set(metal_layer.clone()).unwrap();

            Retained::<CALayer>::from(&metal_layer)
        }

        #[unsafe(method(setFrameSize:))]
        unsafe fn set_frame_size(&self, new_size: NSSize) {
            unsafe {
                let _: () = msg_send![super(self), setFrameSize: new_size];
            }

            let metal_layer = self.ivars().metal_layer.get().unwrap();

            let window = &mut *self.ivars().window.borrow_mut();
            let gfx = &mut *self.ivars().gfx.borrow_mut();
            let app = &*self.ivars().app.borrow();

            let last_scale = window.inner.scale;
            window.inner.resize(new_size.width, new_size.height);

            let scale = window.inner.scale;
            let new_size = CGSize::new(window.inner.width, window.inner.height);

            metal_layer.setContentsScale(scale);

            unsafe {
                metal_layer.setDrawableSize(new_size);
            }

            if let Some(gfx) = gfx {
                gfx.inner.resize(new_size.width, new_size.height).unwrap();

                if scale != last_scale {
                    let Config {
                        font, font_size, ..
                    } = app.config();

                    gfx.inner.update_font(font, *font_size, scale as f32);
                }
            }
        }

        #[unsafe(method(viewWillStartLiveResize))]
        unsafe fn view_will_start_live_resize(&self) {
            let metal_layer = self.ivars().metal_layer.get().unwrap();

            unsafe {
                metal_layer.setPresentsWithTransaction(true);
            }
        }

        #[unsafe(method(viewDidEndLiveResize))]
        unsafe fn view_did_end_live_resize(&self) {
            let metal_layer = self.ivars().metal_layer.get().unwrap();

            unsafe {
                metal_layer.setPresentsWithTransaction(false);
            }
        }

        #[unsafe(method(viewDidChangeBackingProperties))]
        unsafe fn view_did_change_backing_properties(&self) {
            let metal_layer = self.ivars().metal_layer.get().unwrap();

            let scale = metal_layer.contentsScale();
            let size = unsafe { metal_layer.drawableSize() };
            let size = CGSize::new(size.width / scale, size.height / scale);

            unsafe {
                self.setFrameSize(size);
            }

            let mut gfx = self.ivars().gfx.borrow_mut();
            let gfx = gfx.as_mut().unwrap();
            let view = gfx.inner.view();

            unsafe {
                view.setNeedsDisplay(true);
            }
        }

        #[unsafe(method(acceptsFirstResponder))]
        unsafe fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(keyDown:))]
        unsafe fn key_down(&self, event: &NSEvent) {
            handle_event!(handle_key_down, self, event);
        }

        #[unsafe(method(mouseDown:))]
        unsafe fn mouse_down(&self, event: &NSEvent) {
            handle_event!(handle_mouse_down, self, event, false);
        }

        #[unsafe(method(rightMouseDown:))]
        unsafe fn right_mouse_down(&self, event: &NSEvent) {
            handle_event!(handle_mouse_down, self, event, false);
        }

        #[unsafe(method(otherMouseDown:))]
        unsafe fn other_mouse_down(&self, event: &NSEvent) {
            handle_event!(handle_mouse_down, self, event, false);
        }

        #[unsafe(method(mouseUp:))]
        unsafe fn mouse_up(&self, event: &NSEvent) {
            handle_event!(handle_mouse_up, self, event);
        }

        #[unsafe(method(rightMouseUp:))]
        unsafe fn right_mouse_up(&self, event: &NSEvent) {
            handle_event!(handle_mouse_up, self, event);
        }

        #[unsafe(method(otherMouseUp:))]
        unsafe fn other_mouse_up(&self, event: &NSEvent) {
            handle_event!(handle_mouse_up, self, event);
        }

        #[unsafe(method(mouseDragged:))]
        unsafe fn mouse_dragged(&self, event: &NSEvent) {
            handle_event!(handle_mouse_down, self, event, true);
        }

        #[unsafe(method(mouseMoved:))]
        unsafe fn mouse_moved(&self, event: &NSEvent) {
            handle_event!(handle_mouse_down, self, event, true);
        }

        #[unsafe(method(scrollWheel:))]
        unsafe fn scroll_wheel(&self, event: &NSEvent) {
            handle_event!(handle_scroll_wheel, self, event);
        }

        #[unsafe(method(update))]
        fn update_objc(&self) {
            self.update();
        }
    }

    unsafe impl NSObjectProtocol for View {}

    unsafe impl CALayerDelegate for View {
        #[unsafe(method(displayLayer:))]
        unsafe fn display_layer(&self, _layer: &CALayer) {
            let Ok(mut window) = self.ivars().window.try_borrow_mut() else {
                return;
            };

            let Ok(mut gfx) = self.ivars().gfx.try_borrow_mut() else {
                return;
            };

            let Some(gfx) = gfx.as_mut() else {
                return;
            };

            let Ok(mut app) = self.ivars().app.try_borrow_mut() else {
                return;
            };

            let window = &mut *window;

            app.draw(window, gfx);

            if !window.inner.was_shown {
                window.inner.ns_window.makeKeyAndOrderFront(None);
                window.inner.was_shown = true;
            }

            unsafe {
                gfx.inner.display_link.setPaused(!app.is_animating());
            }
        }
    }
);

impl View {
    pub fn new(
        app: Rc<RefCell<App>>,
        window: Rc<RefCell<AnyWindow>>,
        gfx: Rc<RefCell<Option<AnyGfx>>>,
        mtm: MainThreadMarker,
        frame_rect: NSRect,
        device: Retained<ProtocolObject<dyn MTLDevice>>,
    ) -> Retained<Self> {
        let this = mtm.alloc();
        let this = this.set_ivars(ViewIvars {
            app,
            window,
            gfx,
            device,
            metal_layer: OnceCell::new(),
        });

        let view: Retained<View> = unsafe {
            msg_send![
                super(this),
                initWithFrame: frame_rect
            ]
        };

        view.setWantsLayer(true);

        unsafe {
            view.setLayerContentsRedrawPolicy(NSViewLayerContentsRedrawPolicy::DuringViewResize);

            view.setLayerContentsPlacement(NSViewLayerContentsPlacement::ScaleAxesIndependently);
        }

        view
    }

    pub fn update(&self) {
        let Ok(mut window) = self.ivars().window.try_borrow_mut() else {
            return;
        };

        let Ok(mut gfx) = self.ivars().gfx.try_borrow_mut() else {
            return;
        };

        let Some(gfx) = gfx.as_mut() else {
            return;
        };

        let Ok(mut app) = self.ivars().app.try_borrow_mut() else {
            return;
        };

        let window = &mut *window;

        let timestamp = window.inner.get_time(app.is_animating());
        app.update(window, gfx, timestamp);

        let (file_watcher, files, ptys) = app.files_and_ptys();
        window.inner.update(file_watcher, files, ptys);

        unsafe {
            self.setNeedsDisplay(true);
        }
    }

    pub unsafe fn next_drawable(&self) -> Option<Retained<ProtocolObject<dyn CAMetalDrawable>>> {
        let metal_layer = self.ivars().metal_layer.get().unwrap();

        unsafe { metal_layer.nextDrawable() }
    }
}

// SAFETY: It's only ok to use the view to trigger an update,
// and only once an NSThread has been created to signal to Cocoa
// that multi-threading is used.
unsafe impl Send for ViewRef {}
unsafe impl Sync for ViewRef {}

pub struct ViewRef {
    inner: Retained<View>,
}

impl ViewRef {
    pub fn new(inner: &Retained<View>) -> Self {
        Self {
            inner: inner.clone(),
        }
    }

    pub unsafe fn update(&self) {
        unsafe {
            self.inner
                .performSelectorOnMainThread_withObject_waitUntilDone(sel!(update), None, false);
        }
    }
}
