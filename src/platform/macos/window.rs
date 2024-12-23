use std::{cell::RefCell, path::Path, ptr::NonNull, rc::Rc};

use objc2::{rc::Retained, runtime::ProtocolObject, sel};
use objc2_app_kit::*;
use objc2_foundation::*;

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
    platform::aliases::{AnyFileWatcher, AnyGfx, AnyPty},
    temp_buffer::TempBuffer,
};

use super::{delegate::Delegate, file_watcher::FileWatcher, result::Result};

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

        let appearance = NSAppearance::appearanceNamed(appearance_name);

        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        app.setAppearance(appearance.as_deref());

        let menubar = NSMenu::new(mtm);
        let app_menu_item = NSMenuItem::new(mtm);
        menubar.addItem(&app_menu_item);
        app.setMainMenu(Some(&menubar));

        let app_menu = NSMenu::new(mtm);
        let quit_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                ns_string!("Quit Keylime"),
                Some(sel!(terminate:)),
                ns_string!("q"),
            )
        };
        app_menu.addItem(&quit_item);
        app_menu_item.setSubmenu(Some(&app_menu));

        let delegate = Delegate::new(self.app.clone(), mtm);
        let object = ProtocolObject::from_ref(&*delegate);
        app.setDelegate(Some(object));
        app.run();
    }
}

#[derive(Clone, Copy, Debug)]
struct RecordedMouseClick {
    button: MouseButton,
    kind: MouseClickKind,
}

pub struct Window {
    pub ns_window: Retained<NSWindow>,
    pub width: f64,
    pub height: f64,

    pub was_shown: bool,
    pub is_focused: bool,
    pub is_running: bool,
    pub time: f32,
    last_queried_time: Option<f64>,

    pub gfx: Option<AnyGfx>,
    scale: f32,
    file_watcher: AnyFileWatcher,

    wide_text_buffer: TempBuffer<u16>,
    text_buffer: TempBuffer<char>,

    pub chars_typed: Vec<char>,
    pub keybinds_typed: Vec<Keybind>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,

    current_pressed_button: Option<RecordedMouseClick>,

    implicit_copy_change_count: Option<isize>,
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
            is_running: true,
            time: 0.0,
            last_queried_time: None,

            gfx: None,
            scale,
            file_watcher: AnyFileWatcher {
                inner: FileWatcher::new(),
            },

            wide_text_buffer: TempBuffer::new(),
            text_buffer: TempBuffer::new(),

            chars_typed: Vec::new(),
            keybinds_typed: Vec::new(),
            mousebinds_pressed: Vec::new(),
            mouse_scrolls: Vec::new(),

            current_pressed_button: None,

            implicit_copy_change_count: None,
        }
    }

    pub fn resize(&mut self, width: f64, height: f64, app: &App) {
        self.width = width;
        self.height = height;

        if let Some(gfx) = &mut self.gfx {
            gfx.inner.resize(width, height).unwrap();
        }

        let scale = self.ns_window.backingScaleFactor() as f32;

        if scale != self.scale {
            self.scale = scale;

            if let Some(gfx) = &mut self.gfx {
                let Config {
                    font, font_size, ..
                } = app.config();

                gfx.inner.update_font(font, *font_size, self.scale);
            }
        }
    }

    pub fn get_time(&mut self, is_animating: bool) -> (f32, f32) {
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

    pub fn update<'a>(
        &mut self,
        files: impl Iterator<Item = &'a Path>,
        ptys: impl Iterator<Item = &'a mut AnyPty>,
    ) {
        self.clear_inputs();

        if let Some(gfx) = &self.gfx {
            let view = gfx.inner.view();

            for pty in ptys {
                pty.inner.try_start(view);
            }

            self.file_watcher.inner.try_start(view);
        }

        self.file_watcher.inner.update(files).unwrap();
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

        if let Some(key) = Self::key_from_keycode(key_code) {
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

    pub fn gfx(&mut self) -> &mut AnyGfx {
        self.gfx.as_mut().unwrap()
    }

    pub fn file_watcher(&mut self) -> &mut AnyFileWatcher {
        &mut self.file_watcher
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
        let wide_text_buffer = self.wide_text_buffer.get_mut();

        for c in text {
            let mut dst = [0u16; 2];

            for wide_c in c.encode_utf16(&mut dst) {
                wide_text_buffer.push(*wide_c);
            }
        }

        let wide_text_ptr = NonNull::new(wide_text_buffer.as_mut_ptr()).unwrap();

        let mtm = MainThreadMarker::new().unwrap();

        let text = unsafe {
            NSString::initWithCharacters_length(mtm.alloc(), wide_text_ptr, wide_text_buffer.len())
        };

        let protocol_object = ProtocolObject::from_retained(text);
        let protocol_objects = NSArray::from_retained_slice(&[protocol_object]);

        let pasteboard = unsafe { NSPasteboard::generalPasteboard() };

        let did_succeed = unsafe {
            pasteboard.clearContents();
            pasteboard.writeObjects(&protocol_objects)
        };

        if !did_succeed {
            return Err("Failed to write to pasteboard");
        }

        if was_copy_implicit {
            let change_count = unsafe { pasteboard.changeCount() };
            self.implicit_copy_change_count = Some(change_count);
        }

        Ok(())
    }

    pub fn get_clipboard(&mut self) -> Result<&[char]> {
        let text = unsafe {
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.stringForType(NSPasteboardTypeString)
        };

        let Some(text) = text else {
            return Err("Failed to get pasteboard content");
        };

        let wide_text_buffer = self.wide_text_buffer.get_mut();
        let text_buffer = self.text_buffer.get_mut();

        for i in 0..text.length() {
            let wide_char = unsafe { text.characterAtIndex(i) };

            wide_text_buffer.push(wide_char);
        }

        for c in char::decode_utf16(wide_text_buffer.iter().copied()) {
            let Ok(c) = c else {
                continue;
            };

            text_buffer.push(c);
        }

        Ok(text_buffer)
    }

    pub fn was_copy_implicit(&self) -> bool {
        let change_count = unsafe {
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.changeCount()
        };

        self.implicit_copy_change_count == Some(change_count)
    }

    fn key_from_keycode(value: u16) -> Option<Key> {
        match value {
            0x00 => Some(Key::A),
            0x01 => Some(Key::S),
            0x02 => Some(Key::D),
            0x03 => Some(Key::F),
            0x04 => Some(Key::H),
            0x05 => Some(Key::G),
            0x06 => Some(Key::Z),
            0x07 => Some(Key::X),
            0x08 => Some(Key::C),
            0x09 => Some(Key::V),
            0x0B => Some(Key::B),
            0x0C => Some(Key::Q),
            0x0D => Some(Key::W),
            0x0E => Some(Key::E),
            0x0F => Some(Key::R),
            0x10 => Some(Key::Y),
            0x11 => Some(Key::T),
            0x12 => Some(Key::One),
            0x13 => Some(Key::Two),
            0x14 => Some(Key::Three),
            0x15 => Some(Key::Four),
            0x16 => Some(Key::Six),
            0x17 => Some(Key::Five),
            0x19 => Some(Key::Nine),
            0x1A => Some(Key::Seven),
            0x1C => Some(Key::Eight),
            0x1D => Some(Key::Zero),
            0x1E => Some(Key::RBracket),
            0x1F => Some(Key::O),
            0x20 => Some(Key::U),
            0x21 => Some(Key::LBracket),
            0x22 => Some(Key::I),
            0x23 => Some(Key::P),
            0x25 => Some(Key::L),
            0x26 => Some(Key::J),
            0x28 => Some(Key::K),
            0x2C => Some(Key::ForwardSlash),
            0x2D => Some(Key::N),
            0x2E => Some(Key::M),
            0x32 => Some(Key::Grave),
            0x24 => Some(Key::Enter),
            0x30 => Some(Key::Tab),
            0x31 => Some(Key::Space),
            0x33 => Some(Key::Backspace),
            0x35 => Some(Key::Escape),
            // 0x37 => Some(Key::Command),
            0x38 => Some(Key::Shift),
            // 0x3A => Some(Key::Option),
            0x3B => Some(Key::Control),
            // 0x3C => Some(Key::RightShift),
            // 0x3D => Some(Key::RightOption),
            // 0x3E => Some(Key::RightControl),
            0x60 => Some(Key::F5),
            0x61 => Some(Key::F6),
            0x62 => Some(Key::F7),
            0x63 => Some(Key::F3),
            0x64 => Some(Key::F8),
            0x65 => Some(Key::F9),
            0x67 => Some(Key::F11),
            0x6D => Some(Key::F10),
            0x6F => Some(Key::F12),
            0x72 => Some(Key::Help),
            0x73 => Some(Key::Home),
            0x74 => Some(Key::PageUp),
            0x75 => Some(Key::Delete),
            0x76 => Some(Key::F4),
            0x77 => Some(Key::End),
            0x78 => Some(Key::F2),
            0x79 => Some(Key::PageDown),
            0x7A => Some(Key::F1),
            0x7B => Some(Key::Left),
            0x7C => Some(Key::Right),
            0x7D => Some(Key::Down),
            0x7E => Some(Key::Up),
            _ => None,
        }
    }
}
