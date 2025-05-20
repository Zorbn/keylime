use std::{path::Path, ptr::NonNull};

use objc2::{
    rc::{Retained, Weak},
    runtime::ProtocolObject,
};
use objc2_app_kit::*;
use objc2_foundation::*;

use crate::{
    config::theme::Theme,
    geometry::visual_position::VisualPosition,
    input::{
        input_handlers::{GraphemeHandler, KeybindHandler, MouseScrollHandler, MousebindHandler},
        key::Key,
        keybind::Keybind,
        mods::{Mod, Mods},
        mouse_button::MouseButton,
        mouse_scroll::MouseScroll,
        mousebind::{MouseClickCount, Mousebind, MousebindKind},
    },
    platform::aliases::{AnyFileWatcher, AnyProcess, AnyWindow},
    pool::UTF16_POOL,
    text::grapheme::GraphemeCursor,
};

use super::{result::Result, view::View};

const DEFAULT_WIDTH: f64 = 768.0;
const DEFAULT_HEIGHT: f64 = 768.0;

#[derive(Clone, Copy, Debug)]
struct RecordedMouseClick {
    button: MouseButton,
    count: MouseClickCount,
}

pub struct Window {
    pub ns_window: Retained<NSWindow>,
    pub view: Weak<View>,
    pub width: f64,
    pub height: f64,
    pub scale: f64,

    pub was_shown: bool,
    pub is_focused: bool,
    pub time: f32,
    last_queried_time: Option<f64>,

    pub graphemes_typed: String,
    pub grapheme_cursor: GraphemeCursor,
    pub keybinds_typed: Vec<Keybind>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,
    mods: Mods,

    was_last_scroll_horizontal: bool,
    current_pressed_button: Option<RecordedMouseClick>,

    implicit_copy_change_count: Option<isize>,
}

impl Window {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let content_rect = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT),
        );

        let style = NSWindowStyleMask::Closable
            | NSWindowStyleMask::Resizable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Titled;

        let ns_window = unsafe {
            let ns_window = NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc(),
                content_rect,
                style,
                NSBackingStoreType::Buffered,
                false,
            );

            ns_window.setReleasedWhenClosed(false);

            ns_window
        };

        let scale = ns_window.backingScaleFactor();

        Self {
            ns_window,
            view: Weak::default(),
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,

            was_shown: false,
            is_focused: true,
            time: 0.0,
            last_queried_time: None,

            scale,

            graphemes_typed: String::new(),
            grapheme_cursor: GraphemeCursor::new(0, 0),
            keybinds_typed: Vec::new(),
            mousebinds_pressed: Vec::new(),
            mouse_scrolls: Vec::new(),
            mods: Mods::NONE,

            was_last_scroll_horizontal: false,
            current_pressed_button: None,

            implicit_copy_change_count: None,
        }
    }

    pub fn set_theme(&mut self, theme: &Theme) {
        let r = theme.background.r as f64 / 255.0f64;
        let g = theme.background.g as f64 / 255.0f64;
        let b = theme.background.b as f64 / 255.0f64;
        let a = theme.background.a as f64 / 255.0f64;

        // Setting the background color also tints the titlebar to help it match the content.
        unsafe {
            self.ns_window
                .setBackgroundColor(Some(&NSColor::colorWithRed_green_blue_alpha(r, g, b, a)));
        }

        let appearance_name = unsafe {
            if theme.is_dark() {
                NSAppearanceNameDarkAqua
            } else {
                NSAppearanceNameAqua
            }
        };

        let appearance = NSAppearance::appearanceNamed(appearance_name);

        unsafe {
            self.ns_window.setAppearance(appearance.as_deref());
        }
    }

    pub fn resize(&mut self, width: f64, height: f64) {
        let scale = self.ns_window.backingScaleFactor();

        self.scale = scale;
        self.width = width * scale;
        self.height = height * scale;
    }

    pub fn time(&mut self, is_animating: bool) -> (f32, f32) {
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
        file_watcher: &mut AnyFileWatcher,
        files: impl Iterator<Item = &'a Path>,
        processes: impl Iterator<Item = &'a mut AnyProcess>,
    ) {
        self.clear_inputs();

        for process in processes {
            process.inner.try_start(&self.view);
        }

        file_watcher.inner.try_start(&self.view);

        file_watcher.inner.update(files).unwrap();
    }

    fn clear_inputs(&mut self) {
        self.graphemes_typed.clear();
        self.grapheme_cursor = GraphemeCursor::new(0, 0);
        self.keybinds_typed.clear();
        self.mousebinds_pressed.clear();
        self.mouse_scrolls.clear();
    }

    pub fn handle_key_down(&mut self, event: &NSEvent) {
        let modifier_flags = unsafe { event.modifierFlags() };

        if modifier_flags
            .intersection(
                NSEventModifierFlags::Command
                    | NSEventModifierFlags::Control
                    | NSEventModifierFlags::Function
                    | NSEventModifierFlags::Option,
            )
            .is_empty()
        {
            if let Some(chars) = unsafe { event.characters() } {
                self.handle_chars(chars);
            }
        }

        let key_code = unsafe { event.keyCode() };

        if let Some(key) = Self::key_from_keycode(key_code) {
            let mods = Self::modifier_flags_to_mods(modifier_flags);
            self.keybinds_typed.push(Keybind::new(key, mods));
        }
    }

    pub fn handle_flags_changed(&mut self, event: &NSEvent) {
        let modifier_flags = unsafe { event.modifierFlags() };
        let mods = Self::modifier_flags_to_mods(modifier_flags);

        self.mods = mods;
    }

    pub fn handle_mouse_down(&mut self, event: &NSEvent, is_drag: bool) {
        let (x, y) = self.event_location_to_xy(event);

        let modifier_flags = unsafe { event.modifierFlags() };
        let mods = Self::modifier_flags_to_mods(modifier_flags);

        let (button, count, kind) = if is_drag {
            self.current_pressed_button
                .map(|click| (Some(click.button), click.count, MousebindKind::Move))
                .unwrap_or((None, MouseClickCount::Single, MousebindKind::Move))
        } else {
            let click_count = unsafe { event.clickCount() - 1 } % 3 + 1;

            let count = match click_count {
                1 => MouseClickCount::Single,
                2 => MouseClickCount::Double,
                3 => MouseClickCount::Triple,
                _ => unreachable!(),
            };

            let button = Self::get_event_button(event);

            if let Some(button) = button {
                self.current_pressed_button = Some(RecordedMouseClick { button, count });
            }

            (button, count, MousebindKind::Press)
        };

        self.mousebinds_pressed
            .push(Mousebind::new(button, x, y, mods, count, kind));
    }

    pub fn handle_mouse_up(&mut self, event: &NSEvent) {
        let (x, y) = self.event_location_to_xy(event);

        let modifier_flags = unsafe { event.modifierFlags() };
        let mods = Self::modifier_flags_to_mods(modifier_flags);

        let button = Self::get_event_button(event);

        if button == self.current_pressed_button.map(|click| click.button) {
            self.current_pressed_button = None;
        }

        self.mousebinds_pressed.push(Mousebind::new(
            button,
            x,
            y,
            mods,
            MouseClickCount::Single,
            MousebindKind::Release,
        ));
    }

    fn modifier_flags_to_mods(modifier_flags: NSEventModifierFlags) -> Mods {
        let mut mods = Mods::NONE;

        if modifier_flags.contains(NSShiftKeyMask) {
            mods = mods.with(Mod::Shift);
        }

        if modifier_flags.contains(NSControlKeyMask) {
            mods = mods.with(Mod::Ctrl);
        }

        if modifier_flags.contains(NSAlternateKeyMask) {
            mods = mods.with(Mod::Alt);
        }

        if modifier_flags.contains(NSCommandKeyMask) {
            mods = mods.with(Mod::Cmd);
        }

        mods
    }

    pub fn handle_scroll_wheel(&mut self, event: &NSEvent) {
        let (x, y) = self.event_location_to_xy(event);

        let is_precise = unsafe { event.hasPreciseScrollingDeltas() };

        let delta_x = unsafe { -event.scrollingDeltaX() } as f32;
        let delta_y = unsafe { event.scrollingDeltaY() } as f32;

        const INERTIA: f32 = 2.0;

        let is_horizontal = if self.was_last_scroll_horizontal {
            delta_x.abs() * INERTIA > delta_y.abs()
        } else {
            delta_x.abs() > delta_y.abs() * INERTIA
        };

        let delta = if is_horizontal { delta_x } else { delta_y };

        self.was_last_scroll_horizontal = is_horizontal;

        self.mouse_scrolls.push(MouseScroll {
            delta,
            is_horizontal,
            is_precise,
            x,
            y,
        });
    }

    fn event_location_to_xy(&self, event: &NSEvent) -> (f32, f32) {
        let point = unsafe { event.locationInWindow() };

        self.point_in_window_to_xy(point)
    }

    fn point_in_window_to_xy(&self, point: NSPoint) -> (f32, f32) {
        let x = point.x * self.scale;
        let y = self.height - (point.y * self.scale);

        (x as f32, y as f32)
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
        let mut wide_text = UTF16_POOL.new_item();

        for i in 0..chars.length() {
            let wide_char = unsafe { chars.characterAtIndex(i) };

            wide_text.push(wide_char);
        }

        for c in char::decode_utf16(wide_text.iter().copied()) {
            let Ok(c) = c else {
                continue;
            };

            if c.is_control() || matches!(c, '\u{f700}'..='\u{f703}') {
                continue;
            }

            self.graphemes_typed.push(c);
        }

        self.grapheme_cursor =
            GraphemeCursor::new(self.grapheme_cursor.index(), self.graphemes_typed.len());
    }

    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    pub fn grapheme_handler(&self) -> GraphemeHandler {
        GraphemeHandler::new(self.grapheme_cursor.clone())
    }

    pub fn keybind_handler(&self) -> KeybindHandler {
        KeybindHandler::new(self.keybinds_typed.len())
    }

    pub fn mousebind_handler(&self) -> MousebindHandler {
        MousebindHandler::new(self.mousebinds_pressed.len())
    }

    pub fn mouse_scroll_handler(&self) -> MouseScrollHandler {
        MouseScrollHandler::new(self.mouse_scrolls.len())
    }

    pub fn mouse_position(&self) -> VisualPosition {
        let point = unsafe { self.ns_window.mouseLocationOutsideOfEventStream() };
        let (x, y) = self.point_in_window_to_xy(point);

        VisualPosition::new(x, y)
    }

    pub fn mods(&self) -> Mods {
        self.mods
    }

    pub fn set_clipboard(&mut self, text: &str, was_copy_implicit: bool) -> Result<()> {
        let mut wide_text = UTF16_POOL.new_item();

        for c in text.chars() {
            if !AnyWindow::is_char_copy_pastable(c) {
                continue;
            }

            let mut dst = [0u16; 2];

            for wide_c in c.encode_utf16(&mut dst) {
                wide_text.push(*wide_c);
            }
        }

        let wide_text_ptr = NonNull::new(wide_text.as_mut_ptr()).unwrap();

        let mtm = MainThreadMarker::new().unwrap();

        let text = unsafe {
            NSString::initWithCharacters_length(mtm.alloc(), wide_text_ptr, wide_text.len())
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

    pub fn get_clipboard(&self, text: &mut String) -> Result<()> {
        let pasteboard_string = unsafe {
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.stringForType(NSPasteboardTypeString)
        };

        let Some(pasteboard_string) = pasteboard_string else {
            return Err("Failed to get pasteboard content");
        };

        let mut wide_text = UTF16_POOL.new_item();

        for i in 0..pasteboard_string.length() {
            let wide_char = unsafe { pasteboard_string.characterAtIndex(i) };

            wide_text.push(wide_char);
        }

        for c in char::decode_utf16(wide_text.iter().copied()) {
            let Ok(c) = c else {
                continue;
            };

            if !AnyWindow::is_char_copy_pastable(c) {
                continue;
            }

            text.push(c);
        }

        Ok(())
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
            0x1B => Some(Key::Minus),
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
            0x2F => Some(Key::Period),
            0x32 => Some(Key::Grave),
            0x24 => Some(Key::Enter),
            0x30 => Some(Key::Tab),
            0x31 => Some(Key::Space),
            0x33 => Some(Key::Backspace),
            0x35 => Some(Key::Escape),
            0x37 => Some(Key::Cmd),
            0x38 => Some(Key::Shift),
            0x3A => Some(Key::Alt),
            0x3B => Some(Key::Ctrl),
            0x3C => Some(Key::RShift),
            0x3D => Some(Key::RAlt),
            0x3E => Some(Key::RCtrl),
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
