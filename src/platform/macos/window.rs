#![deny(unsafe_op_in_unsafe_fn)]

use std::{cell::RefCell, path::Path, rc::Rc};

use objc2::{
    declare_class, msg_send, msg_send_id, mutability::MainThreadOnly, rc::Retained,
    runtime::ProtocolObject, ClassType, DeclaredClass,
};
use objc2_app_kit::*;
use objc2_foundation::{
    ns_string, MainThreadMarker, NSDate, NSNotification, NSObject, NSObjectProtocol, NSPoint,
    NSRect, NSSize, NSString, NSThread,
};
use objc2_metal_kit::{MTKView, MTKViewDelegate};

use crate::{
    app::App,
    config::Config,
    input::{
        input_handlers::{CharHandler, KeybindHandler, MouseScrollHandler, MousebindHandler},
        key::Key,
        keybind::Keybind,
        mouse_button::MouseButton,
        mouse_scroll::MouseScroll,
        mousebind::{MouseClickKind, Mousebind},
    },
    temp_buffer::TempBuffer,
};

use super::{file_watcher::FileWatcher, gfx::Gfx, pty::Pty, result::Result};

struct Ivars {
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
            let app = self.ivars().app.clone();

            let mtm = MainThreadMarker::from(self);

            let (ns_window, width, height) = {
                let window = window.borrow();

                (window.ns_window.clone(), window.width, window.height)
            };

            unsafe {
                let app = app.borrow();

                let theme = &app.config().theme;

                let r = theme.background.r as f64 / 255.0f64;
                let g = theme.background.g as f64 / 255.0f64;
                let b = theme.background.b as f64 / 255.0f64;
                let a = theme.background.a as f64 / 255.0f64;

                ns_window.setBackgroundColor(Some(&NSColor::colorWithRed_green_blue_alpha(r, g, b, a)));
                ns_window.setAcceptsMouseMovedEvents(true);
            }

            let protocol_object = ProtocolObject::from_ref(self);
            ns_window.setDelegate(Some(&protocol_object));

            let mut gfx = unsafe {
                let protocol_object = ProtocolObject::from_ref(self);

                let app = app.borrow();

                let Config {
                    font, font_size, ..
                } = app.config();

                Gfx::new(font, *font_size, window.clone(), &ns_window, mtm, protocol_object).unwrap()
            };

            gfx.resize(width, height).unwrap();

            let view = gfx.view().clone();

            window.borrow_mut().gfx = Some(gfx);

            ns_window.setContentView(Some(&view));
            ns_window.center();
            ns_window.setTitle(ns_string!("Keylime"));

            // Create a blank thread to tell Cocoa that we are using multi-threading (for the pty).
            // https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/CreatingThreads/CreatingThreads.html
            let _ = NSThread::new();
            assert!(NSThread::isMultiThreaded());

            unsafe {
                let app: &mut NSApplication = msg_send![_notification, object];
                app.activate();
                view.setNeedsDisplay(true);
            }
        }

        #[method(applicationShouldTerminateAfterLastWindowClosed:)]
        #[allow(non_snake_case)]
        unsafe fn applicationShouldTerminateAfterLastWindowClosed(&self, _sender: &NSApplication) -> bool {
            true
        }
    }

    unsafe impl NSWindowDelegate for Delegate {
        #[method(windowShouldClose:)]
        #[allow(non_snake_case)]
        unsafe fn windowShouldClose(&self, _sender: &NSWindow) -> bool {
            let time = self.ivars().window.borrow().time;
            let mut app = self.ivars().app.borrow_mut();

            app.close(time);

            true
        }

        #[method(windowDidBecomeKey:)]
        #[allow(non_snake_case)]
        unsafe fn windowDidBecomeKey(&self, _notification: &NSNotification) {
            self.on_focused_changed(true);
        }

        #[method(windowDidResignKey:)]
        #[allow(non_snake_case)]
        unsafe fn windowDidResignKey(&self, _notification: &NSNotification) {
            self.on_focused_changed(false);
        }

        #[method(windowDidEnterFullScreen:)]
        #[allow(non_snake_case)]
        unsafe fn windowDidEnterFullScreen(&self, _notification: &NSNotification) {
            self.on_fullscreen_changed(true);
        }

        #[method(windowDidExitFullScreen:)]
        #[allow(non_snake_case)]
        unsafe fn windowDidExitFullScreen(&self, _notification: &NSNotification) {
            self.on_fullscreen_changed(false);
        }
    }

    unsafe impl MTKViewDelegate for Delegate {
        #[method(drawInMTKView:)]
        #[allow(non_snake_case)]
        unsafe fn drawInMTKView(&self, _view: &MTKView) {
            let Ok(mut window) = self.ivars().window.try_borrow_mut() else {
                return;
            };

            let Ok(mut app) = self.ivars().app.try_borrow_mut() else {
                return;
            };

            let window = &mut *window;

            app.update(window);
            window.clear_inputs();
            app.draw(window);

            if !window.was_shown {
                window.ns_window.makeKeyAndOrderFront(None);
                window.was_shown = true;
            }

            if app.is_animating() {
                let gfx = window.gfx();

                unsafe {
                    gfx.view().setNeedsDisplay(true);
                }
            }
        }

        #[method(mtkView:drawableSizeWillChange:)]
        #[allow(non_snake_case)]
        unsafe fn mtkView_drawableSizeWillChange(&self, view: &MTKView, size: NSSize) {
            let window = &mut *self.ivars().window.borrow_mut();
            let app = &*self.ivars().app.borrow();

            window.resize(size.width, size.height, app);

            unsafe {
                view.setNeedsDisplay(true);
            }
        }
    }
);

impl Delegate {
    fn new(app: Rc<RefCell<App>>, mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc();
        let this = this.set_ivars(Ivars {
            window: Rc::new(RefCell::new(Window::new(mtm, 768.0, 768.0))),
            app,
        });

        unsafe { msg_send_id![super(this), init] }
    }

    fn on_focused_changed(&self, is_focused: bool) {
        let Ok(mut window) = self.ivars().window.try_borrow_mut() else {
            return;
        };

        window.is_focused = is_focused;

        let view = window.gfx().view();

        unsafe {
            view.setNeedsDisplay(true);
        }
    }

    fn on_fullscreen_changed(&self, is_fullscreen: bool) {
        let Ok(mut window) = self.ivars().window.try_borrow_mut() else {
            return;
        };

        window.gfx().is_fullscreen = is_fullscreen;
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

#[derive(Clone, Copy, Debug)]
struct RecordedMouseClick {
    button: MouseButton,
    kind: MouseClickKind,
}

pub struct Window {
    ns_window: Retained<NSWindow>,
    width: f64,
    height: f64,

    was_shown: bool,
    is_focused: bool,
    time: f32,
    last_queried_time: Option<f64>,

    gfx: Option<Gfx>,
    scale: f32,
    file_watcher: FileWatcher,

    wide_text_buffer: TempBuffer<u16>,

    pub chars_typed: Vec<char>,
    pub keybinds_typed: Vec<Keybind>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,

    current_pressed_button: Option<RecordedMouseClick>,
}

impl Window {
    pub fn new(mtm: MainThreadMarker, width: f64, height: f64) -> Self {
        let content_rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));

        let ns_window = {
            let style = NSWindowStyleMask::Closable
                | NSWindowStyleMask::Resizable
                | NSWindowStyleMask::Miniaturizable
                | NSWindowStyleMask::Titled;

            unsafe {
                NSWindow::initWithContentRect_styleMask_backing_defer(
                    mtm.alloc(),
                    content_rect,
                    style,
                    NSBackingStoreType::NSBackingStoreBuffered,
                    false,
                )
            }
        };

        let scale = ns_window.backingScaleFactor() as f32;

        Self {
            ns_window,
            width,
            height,

            was_shown: false,
            is_focused: false,
            time: 0.0,
            last_queried_time: None,

            gfx: None,
            scale,
            file_watcher: FileWatcher {},

            wide_text_buffer: TempBuffer::new(),

            chars_typed: Vec::new(),
            keybinds_typed: Vec::new(),
            mousebinds_pressed: Vec::new(),
            mouse_scrolls: Vec::new(),

            current_pressed_button: None,
        }
    }

    fn resize(&mut self, width: f64, height: f64, app: &App) {
        self.width = width;
        self.height = height;

        if let Some(gfx) = &mut self.gfx {
            gfx.resize(width, height).unwrap();
        }

        let scale = self.ns_window.backingScaleFactor() as f32;

        if scale != self.scale {
            self.scale = scale;

            if let Some(gfx) = &mut self.gfx {
                let Config {
                    font, font_size, ..
                } = app.config();

                gfx.update_font(font, *font_size, self.scale);
            }
        }
    }

    pub fn update<'a>(
        &mut self,
        is_animating: bool,
        ptys: impl Iterator<Item = &'a mut Pty>,
        files: impl Iterator<Item = &'a Path>,
    ) -> (f32, f32) {
        if let Some(gfx) = &self.gfx {
            let view = gfx.view();

            for pty in ptys {
                pty.try_start(view);
            }
        }

        let time = unsafe { NSDate::now().timeIntervalSinceReferenceDate() };

        let dt = if let Some(last_queried_time) = self.last_queried_time {
            (time - last_queried_time) as f32
        } else {
            0.0
        };

        self.last_queried_time = Some(time);
        self.time += dt;

        let dt = if is_animating { dt } else { 0.0 };

        (self.time, dt)
    }

    fn clear_inputs(&mut self) {
        self.chars_typed.clear();
        self.keybinds_typed.clear();
        self.mousebinds_pressed.clear();
        self.mouse_scrolls.clear();
    }

    pub fn handle_key_down(&mut self, event: &NSEvent) {
        let modifier_flags = unsafe { event.modifierFlags() };

        if modifier_flags
            .intersection(
                NSEventModifierFlags::NSEventModifierFlagCommand
                    | NSEventModifierFlags::NSEventModifierFlagControl
                    | NSEventModifierFlags::NSEventModifierFlagFunction
                    | NSEventModifierFlags::NSEventModifierFlagOption,
            )
            .is_empty()
        {
            if let Some(chars) = unsafe { event.characters() } {
                self.handle_chars(chars);
            }
        }

        let key_code = unsafe { event.keyCode() };

        if let Some(key) = Key::from_macos_keycode(key_code) {
            // TODO: This just remaps Command -> Ctrl, but really
            // there should be native keybinds for MacOS.
            let has_shift = modifier_flags.contains(NSShiftKeyMask);
            let has_ctrl = modifier_flags.contains(NSCommandKeyMask)
                | modifier_flags.contains(NSControlKeyMask);
            let has_alt = modifier_flags.contains(NSAlternateKeyMask);

            self.keybinds_typed
                .push(Keybind::new(key, has_shift, has_ctrl, has_alt));
        }
    }

    pub fn handle_mouse_down(&mut self, event: &NSEvent, is_drag: bool) {
        let (x, y) = self.event_location_to_xy(event);

        let modifier_flags = unsafe { event.modifierFlags() };
        let has_shift = modifier_flags.contains(NSShiftKeyMask);
        let has_ctrl = modifier_flags.contains(NSCommandKeyMask);
        let has_alt = modifier_flags.contains(NSAlternateKeyMask);

        let (button, kind) = if is_drag {
            self.current_pressed_button
                .map(|click| (Some(click.button), click.kind))
                .unwrap_or((None, MouseClickKind::Single))
        } else {
            let click_count = unsafe { event.clickCount() - 1 } % 3 + 1;

            let kind = match click_count {
                1 => MouseClickKind::Single,
                2 => MouseClickKind::Double,
                3 => MouseClickKind::Triple,
                _ => unreachable!(),
            };

            let button = Self::get_event_button(event);

            if let Some(button) = button {
                self.current_pressed_button = Some(RecordedMouseClick { button, kind });
            }

            (button, kind)
        };

        self.mousebinds_pressed.push(Mousebind::new(
            button, x, y, has_shift, has_ctrl, has_alt, kind, is_drag,
        ));
    }

    pub fn handle_mouse_up(&mut self, event: &NSEvent) {
        let button = Self::get_event_button(event);

        if button == self.current_pressed_button.map(|click| click.button) {
            self.current_pressed_button = None;
        }
    }

    pub fn handle_scroll_wheel(&mut self, event: &NSEvent) {
        let (x, y) = self.event_location_to_xy(event);

        let is_precise = unsafe { event.hasPreciseScrollingDeltas() };

        let delta_x = unsafe { -event.scrollingDeltaX() } as f32;
        let delta_y = unsafe { event.scrollingDeltaY() } as f32;

        let (delta, is_horizontal) = if delta_y.abs() > delta_x.abs() {
            (delta_y, false)
        } else {
            (delta_x, true)
        };

        self.mouse_scrolls.push(MouseScroll {
            delta,
            is_horizontal,
            is_precise,
            x,
            y,
        });
    }

    fn event_location_to_xy(&mut self, event: &NSEvent) -> (f32, f32) {
        let position = unsafe { event.locationInWindow() };
        let x = position.x as f32 * self.scale;
        let y = self.gfx().height() - (position.y as f32 * self.scale);

        (x, y)
    }

    fn get_event_button(event: &NSEvent) -> Option<MouseButton> {
        let button_number = unsafe { event.buttonNumber() };

        match button_number {
            0 => Some(MouseButton::Left),
            1 => Some(MouseButton::Right),
            2 => Some(MouseButton::Middle),
            3 => Some(MouseButton::FirstSide),
            4 => Some(MouseButton::SecondSide),
            _ => None,
        }
    }

    pub fn handle_chars(&mut self, chars: Retained<NSString>) {
        let wide_text_buffer = self.wide_text_buffer.get_mut();

        for i in 0..chars.length() {
            let wide_char = unsafe { chars.characterAtIndex(i) };

            wide_text_buffer.push(wide_char);
        }

        for c in char::decode_utf16(wide_text_buffer.iter().copied()) {
            let Ok(c) = c else {
                continue;
            };

            if c.is_control() || matches!(c, '\u{f700}'..='\u{f703}') {
                continue;
            }

            self.chars_typed.push(c);
        }
    }

    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    pub fn scale(&self) -> f32 {
        self.scale
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
        KeybindHandler::new(self.keybinds_typed.len())
    }

    pub fn get_mousebind_handler(&self) -> MousebindHandler {
        MousebindHandler::new(self.mousebinds_pressed.len())
    }

    pub fn get_mouse_scroll_handler(&self) -> MouseScrollHandler {
        MouseScrollHandler::new(self.mouse_scrolls.len())
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
