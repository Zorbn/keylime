#![deny(unsafe_op_in_unsafe_fn)]

use std::cell::{OnceCell, RefCell};

use objc2::{
    define_class, msg_send,
    rc::{Retained, Weak},
    runtime::ProtocolObject,
    sel, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSEvent, NSView, NSViewLayerContentsPlacement, NSViewLayerContentsRedrawPolicy,
};
use objc2_core_foundation::CGSize;
use objc2_foundation::{
    NSDefaultRunLoopMode, NSObjectNSThreadPerformAdditions, NSObjectProtocol, NSRect, NSRunLoop,
    NSSize,
};
use objc2_metal::MTLDevice;
use objc2_quartz_core::{
    CAAutoresizingMask, CADisplayLink, CALayer, CALayerDelegate, CAMetalDrawable, CAMetalLayer,
};

use crate::{
    app::App,
    config::Config,
    platform::aliases::{AnyGfx, AnyWindow},
};

use super::gfx::PIXEL_FORMAT;

macro_rules! handle_event {
    ($handler:ident, $self:expr, $event:expr $(, $args:expr)*) => {
        if let Ok(mut state) = $self.ivars().state.try_borrow_mut() {
            if let Some(ViewState { window, .. }) = state.as_mut() {
                window.inner.$handler($event, $($args), *);
            }
        }


        if let Some(display_link) = $self.ivars().display_link.get() {
            if unsafe { !display_link.isPaused() } {
                return;
            }
        };

        $self.update();
    };
}

struct ViewState {
    app: App,
    window: AnyWindow,
    gfx: AnyGfx,
}

pub struct ViewIvars {
    state: RefCell<Option<ViewState>>,

    device: Retained<ProtocolObject<dyn MTLDevice>>,
    metal_layer: OnceCell<Retained<CAMetalLayer>>,
    display_link: OnceCell<Retained<CADisplayLink>>,
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

            self.on_set_frame_size(new_size);
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
                self.setNeedsDisplay(true);
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

        #[unsafe(method(flagsChanged:))]
        unsafe fn flags_changed(&self, _event: &NSEvent) {
            self.update();
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
            self.on_display_layer();
        }
    }
);

impl View {
    pub fn new(
        app: App,
        mut window: AnyWindow,
        mut gfx: AnyGfx,
        mtm: MainThreadMarker,
        frame_rect: NSRect,
        device: Retained<ProtocolObject<dyn MTLDevice>>,
    ) -> Retained<Self> {
        let this = mtm.alloc();
        let this = this.set_ivars(ViewIvars {
            state: RefCell::new(None),

            device,
            metal_layer: OnceCell::new(),
            display_link: OnceCell::new(),
        });

        let view: Retained<Self> = unsafe {
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

        window.inner.view = Weak::from_retained(&view);
        gfx.inner.view = Weak::from_retained(&view);

        view.ivars()
            .state
            .replace(Some(ViewState { app, window, gfx }));

        view.ivars()
            .display_link
            .set(unsafe {
                let display_link = view.displayLinkWithTarget_selector(&view, sel!(update));
                display_link
                    .addToRunLoop_forMode(&NSRunLoop::currentRunLoop(), NSDefaultRunLoopMode);

                display_link
            })
            .unwrap();

        view
    }

    pub fn update(&self) -> Option<()> {
        let mut state = self.ivars().state.try_borrow_mut().ok()?;
        let ViewState { app, window, gfx } = state.as_mut()?;

        let is_animating = app.is_animating(window, gfx, window.inner.time);
        let (time, dt) = window.inner.time(is_animating);
        app.update(window, gfx, time, dt);

        let (file_watcher, files, processes) = app.files_and_processes();
        window.inner.update(file_watcher, files, processes);

        unsafe {
            self.setNeedsDisplay(true);
        }

        Some(())
    }

    fn on_set_frame_size(&self, new_size: NSSize) -> Option<()> {
        let mut state = self.ivars().state.try_borrow_mut().ok()?;
        let ViewState { app, window, gfx } = state.as_mut()?;

        let last_scale = window.inner.scale;
        window.inner.resize(new_size.width, new_size.height);

        let scale = window.inner.scale;
        let new_size = CGSize::new(window.inner.width, window.inner.height);

        let metal_layer = self.ivars().metal_layer.get()?;
        metal_layer.setContentsScale(scale);

        unsafe {
            metal_layer.setDrawableSize(new_size);
        }

        gfx.inner.resize(new_size.width, new_size.height).unwrap();

        if scale != last_scale {
            let Config {
                font, font_size, ..
            } = app.config();

            gfx.inner.set_font(font, *font_size, scale as f32);
        }

        Some(())
    }

    fn on_display_layer(&self) -> Option<()> {
        let mut state = self.ivars().state.try_borrow_mut().ok()?;
        let ViewState { app, window, gfx } = state.as_mut()?;

        let time = window.inner.time;

        app.draw(window, gfx, time);

        if !window.inner.was_shown {
            window.inner.ns_window.makeKeyAndOrderFront(None);
            window.inner.was_shown = true;
        }

        let is_animating = app.is_animating(window, gfx, window.inner.time);

        unsafe {
            self.ivars().display_link.get()?.setPaused(!is_animating);
        }

        Some(())
    }

    pub fn on_focused_changed(&self, is_focused: bool) -> Option<()> {
        let mut state = self.ivars().state.try_borrow_mut().ok()?;
        let ViewState { window, .. } = state.as_mut()?;

        window.inner.is_focused = is_focused;

        unsafe {
            self.setNeedsDisplay(true);
        }

        Some(())
    }

    pub fn on_fullscreen_changed(&self, is_fullscreen: bool) -> Option<()> {
        let mut state = self.ivars().state.try_borrow_mut().ok()?;
        let ViewState { gfx, .. } = state.as_mut()?;

        gfx.inner.is_fullscreen = is_fullscreen;

        Some(())
    }

    pub fn on_close(&self) -> Option<()> {
        unsafe {
            self.ivars().display_link.get()?.invalidate();
        }

        let mut state = self.ivars().state.try_borrow_mut().ok()?;
        let ViewState { app, window, gfx } = state.as_mut()?;

        let time = window.inner.time;

        app.close(window, gfx, time);

        state.take();

        Some(())
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
    inner: Weak<View>,
}

impl ViewRef {
    pub fn new(inner: &Weak<View>) -> Self {
        Self {
            inner: inner.clone(),
        }
    }

    pub unsafe fn update(&self) {
        let Some(inner) = self.inner.load() else {
            return;
        };

        unsafe {
            inner.performSelectorOnMainThread_withObject_waitUntilDone(sel!(update), None, false);
        }
    }
}
