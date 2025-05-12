use std::{
    collections::{HashMap, HashSet},
    mem::transmute,
    path::Path,
    ptr::{copy_nonoverlapping, null_mut},
};

use windows::{
    core::Result,
    Win32::{
        Foundation::{
            GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, POINT, RECT, WAIT_OBJECT_0, WPARAM,
        },
        Graphics::{
            Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE},
            Gdi::ScreenToClient,
        },
        System::{
            DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard, SetClipboardData},
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
            Ole::CF_UNICODETEXT,
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
            Threading::INFINITE,
        },
        UI::{
            Input::KeyboardAndMouse::{
                GetDoubleClickTime, GetKeyState, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_RCONTROL,
                VK_RMENU, VK_RSHIFT,
            },
            WindowsAndMessaging::*,
        },
    },
};
use windows_core::BOOL;

use crate::{
    config::theme::Theme,
    geometry::visual_position::VisualPosition,
    input::{
        action::{Action, ActionName},
        input_handlers::{ActionHandler, GraphemeHandler, MouseScrollHandler, MousebindHandler},
        key::Key,
        keybind::Keybind,
        mods::{Mod, Mods},
        mouse_button::MouseButton,
        mouse_scroll::MouseScroll,
        mousebind::{MouseClickCount, MouseClickKind, Mousebind},
    },
    platform::aliases::{AnyFileWatcher, AnyProcess, AnyWindow},
    pool::UTF16_POOL,
    text::grapheme::GraphemeCursor,
};

use super::{deferred_call::defer, keymaps::new_keymaps};

const DEFAULT_WIDTH: i32 = 640;
const DEFAULT_HEIGHT: i32 = 480;

const MK_LBUTTON: usize = 0x01;
const MK_RBUTTON: usize = 0x02;
const MK_MBUTTON: usize = 0x10;
const MK_XBUTTON1: usize = 0x20;
const MK_XBUTTON2: usize = 0x40;
const MK_SHIFT: usize = 0x04;
const MK_CONTROL: usize = 0x08;

#[derive(Clone, Copy, Debug)]
struct RecordedMouseClick {
    button: MouseButton,
    count: MouseClickCount,
    x: f32,
    y: f32,
    time: f32,
}

pub struct Window {
    timer_frequency: i64,
    last_queried_time: Option<i64>,
    pub(super) time: f32,

    hwnd: HWND,

    wait_handles: Vec<HANDLE>,

    is_running: bool,
    is_focused: bool,

    x: i32,
    y: i32,
    pub(super) width: i32,
    pub(super) height: i32,

    // Keep track of which mouse buttons have been pressed since the window was
    // last focused, so that we can skip stray mouse drag events that may happen
    // when the window is gaining focus again after coming back from a popup.
    draggable_buttons: HashSet<MouseButton>,
    current_click: Option<RecordedMouseClick>,
    last_click: Option<RecordedMouseClick>,
    double_click_time: f32,

    keymaps: HashMap<Keybind, ActionName>,
    pub graphemes_typed: String,
    pub grapheme_cursor: GraphemeCursor,
    pub actions_typed: Vec<Action>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,
    mods: Mods,

    was_copy_implicit: bool,
    did_just_copy: bool,
}

impl Window {
    pub(super) fn new() -> Result<Self> {
        let mut timer_frequency = 0i64;
        let triple_click_time;

        unsafe {
            QueryPerformanceFrequency(&mut timer_frequency)?;
            triple_click_time = GetDoubleClickTime() as f32 / 1000.0;
        }

        Ok(Self {
            timer_frequency,
            last_queried_time: None,
            time: 0.0,

            hwnd: HWND(null_mut()),

            wait_handles: Vec::new(),

            is_running: true,
            is_focused: false,

            x: 0,
            y: 0,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,

            draggable_buttons: HashSet::new(),
            current_click: None,
            last_click: None,
            double_click_time: triple_click_time,

            keymaps: new_keymaps(),
            graphemes_typed: String::new(),
            grapheme_cursor: GraphemeCursor::new(0, 0),
            actions_typed: Vec::new(),
            mousebinds_pressed: Vec::new(),
            mouse_scrolls: Vec::new(),
            mods: Mods::NONE,

            was_copy_implicit: false,
            did_just_copy: false,
        })
    }

    pub fn set_theme(&mut self, theme: &Theme) {
        let is_dark = BOOL::from(theme.is_dark());

        unsafe {
            let _ = DwmSetWindowAttribute(
                self.hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &is_dark as *const BOOL as _,
                size_of::<BOOL>() as u32,
            );
        }
    }

    fn clear_inputs(&mut self) {
        self.graphemes_typed.clear();
        self.grapheme_cursor = GraphemeCursor::new(0, 0);
        self.actions_typed.clear();
        self.mousebinds_pressed.clear();
        self.mouse_scrolls.clear();
    }

    pub fn get_time(&mut self, is_animating: bool) -> (f32, f32) {
        unsafe {
            let mut queried_time = 0i64;
            let _ = QueryPerformanceCounter(&mut queried_time);

            let dt = if let Some(last_queried_time) = self.last_queried_time {
                (queried_time - last_queried_time) as f32 / self.timer_frequency as f32
            } else {
                0.0
            };

            self.last_queried_time = Some(queried_time);

            self.time += dt;

            // Don't return massive delta times from big gaps in animation, because those
            // might cause visual jumps or other problems. (eg. if you don't interact with
            // the app for 15 seconds and then you do something that starts an animation,
            // that animation shouldn't instantly jump to completion).
            (self.time, if is_animating { dt } else { 0.0 })
        }
    }

    pub fn update<'a>(
        &mut self,
        is_animating: bool,
        file_watcher: &mut AnyFileWatcher,
        files: impl Iterator<Item = &'a Path>,
        processes: impl Iterator<Item = &'a mut AnyProcess>,
    ) {
        self.clear_inputs();
        file_watcher.inner.update(files).unwrap();

        unsafe {
            let mut msg = MSG::default();

            if !is_animating {
                self.wait_handles.clear();

                for process in processes {
                    self.wait_handles
                        .extend_from_slice(&[process.inner.hprocess, process.inner.event]);
                }

                let dir_handles_start = self.wait_handles.len();

                self.wait_handles.extend(
                    file_watcher
                        .inner
                        .dir_watch_handles()
                        .iter()
                        .map(|handles| handles.event()),
                );

                let result = MsgWaitForMultipleObjects(
                    Some(&self.wait_handles),
                    false,
                    INFINITE,
                    QS_ALLINPUT,
                );

                let index = (result.0 - WAIT_OBJECT_0.0) as usize;

                if index >= dir_handles_start && index < self.wait_handles.len() {
                    file_watcher
                        .inner
                        .handle_dir_update(index - dir_handles_start)
                        .unwrap();
                }
            }

            file_watcher.inner.check_dir_updates().unwrap();

            while PeekMessageW(&mut msg, Some(self.hwnd), 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn get_grapheme_handler(&self) -> GraphemeHandler {
        GraphemeHandler::new(self.grapheme_cursor.clone())
    }

    pub fn get_action_handler(&self) -> ActionHandler {
        ActionHandler::new(self.actions_typed.len())
    }

    pub fn get_mousebind_handler(&self) -> MousebindHandler {
        MousebindHandler::new(self.mousebinds_pressed.len())
    }

    pub fn get_mouse_scroll_handler(&self) -> MouseScrollHandler {
        MouseScrollHandler::new(self.mouse_scrolls.len())
    }

    pub fn get_mouse_position(&self) -> VisualPosition {
        let mut point = POINT::default();

        unsafe {
            let _ = GetCursorPos(&mut point);
            let _ = ScreenToClient(self.hwnd, &mut point);
        }

        VisualPosition::new(point.x as f32, point.y as f32)
    }

    pub fn mods(&self) -> Mods {
        self.mods
    }

    pub fn set_clipboard(&mut self, text: &str, was_copy_implicit: bool) -> Result<()> {
        self.was_copy_implicit = was_copy_implicit;
        self.did_just_copy = true;

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

        wide_text.push(0);

        unsafe {
            OpenClipboard(Some(self.hwnd))?;

            defer!({
                let _ = CloseClipboard();
            });

            let hglobal = GlobalAlloc(GMEM_MOVEABLE, wide_text.len() * size_of::<u16>())?;

            defer!({
                let _ = GlobalFree(Some(hglobal));
            });

            let memory = GlobalLock(hglobal) as *mut u16;

            copy_nonoverlapping(wide_text.as_ptr(), memory, wide_text.len());

            let _ = GlobalUnlock(hglobal);

            SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(hglobal.0)))?;
        }

        Ok(())
    }

    pub fn get_clipboard(&mut self, text: &mut String) -> Result<()> {
        let mut wide_text = UTF16_POOL.new_item();

        unsafe {
            OpenClipboard(Some(self.hwnd))?;

            defer!({
                let _ = CloseClipboard();
            });

            let hglobal = GlobalAlloc(GMEM_MOVEABLE, wide_text.len() * size_of::<u16>())?;

            defer!({
                let _ = GlobalFree(Some(hglobal));
            });

            let hglobal = HGLOBAL(GetClipboardData(CF_UNICODETEXT.0 as u32)?.0);

            let mut memory = GlobalLock(hglobal) as *mut u16;

            if !memory.is_null() {
                while *memory != 0 {
                    wide_text.push(*memory);
                    memory = memory.add(1);
                }
            }

            let _ = GlobalUnlock(hglobal);
        }

        for c in char::decode_utf16(wide_text.iter().copied()) {
            let c = c.unwrap_or('?');

            if !AnyWindow::is_char_copy_pastable(c) {
                continue;
            }

            text.push(c);
        }

        Ok(())
    }

    pub fn was_copy_implicit(&self) -> bool {
        self.was_copy_implicit
    }

    pub(super) unsafe fn on_create(&mut self, scale: f32, hwnd: HWND) {
        self.hwnd = hwnd;

        let mut window_rect = RECT {
            left: 0,
            top: 0,
            right: self.width,
            bottom: self.height,
        };

        AdjustWindowRectEx(
            &mut window_rect,
            WS_OVERLAPPEDWINDOW,
            false,
            WINDOW_EX_STYLE::default(),
        )
        .unwrap();

        let width = ((window_rect.right - window_rect.left) as f32 * scale) as i32;
        let height = ((window_rect.bottom - window_rect.top) as f32 * scale) as i32;

        let _ = SetWindowPos(hwnd, None, 0, 0, width, height, SWP_NOMOVE);
    }

    pub(super) unsafe fn on_dpi_changed(&mut self, rect: RECT) {
        let _ = SetWindowPos(
            self.hwnd,
            None,
            0,
            0,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE,
        );
    }

    pub(super) unsafe fn window_proc(
        &mut self,
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_NCCREATE => {
                let create_struct = lparam.0 as *const CREATESTRUCTW;

                SetWindowLongPtrW(
                    hwnd,
                    GWLP_USERDATA,
                    (*create_struct).lpCreateParams as isize,
                );

                // Update the window to finish setting user data.
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
                );

                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            WM_MOVE => {
                self.x = transmute::<u32, i32>((lparam.0 & 0xFFFF) as u32);
                self.y = transmute::<u32, i32>(((lparam.0 >> 16) & 0xFFFF) as u32);
            }
            WM_CLOSE => {
                self.is_running = false;
            }
            WM_DESTROY => {
                PostQuitMessage(0);
            }
            WM_SETFOCUS => {
                self.is_focused = true;
            }
            WM_KILLFOCUS => {
                self.is_focused = false;
                self.draggable_buttons.clear();
                self.current_click = None;

                let _ = PostMessageW(Some(self.hwnd), WM_PAINT, WPARAM(0), LPARAM(0));
            }
            WM_CHAR => {
                if let Some(c) = char::from_u32(wparam.0 as u32) {
                    if !c.is_control() {
                        self.graphemes_typed.push(c);

                        self.grapheme_cursor = GraphemeCursor::new(
                            self.grapheme_cursor.cur_cursor(),
                            self.graphemes_typed.len(),
                        );
                    }
                }
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                self.mods = Self::key_state_to_mods();

                let Some(key) = Self::key_from_keycode((wparam.0 & 0xFFFF) as u32) else {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                };

                let action = Action::from_keybind(Keybind::new(key, self.mods), &self.keymaps);

                self.actions_typed.push(action);

                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            WM_KEYUP | WM_SYSKEYUP => {
                self.mods = Self::key_state_to_mods();

                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
                let button = match msg {
                    WM_LBUTTONUP => Some(MouseButton::Left),
                    WM_RBUTTONUP => Some(MouseButton::Right),
                    WM_MBUTTONUP => Some(MouseButton::Middle),
                    _ => None,
                };

                if self
                    .current_click
                    .is_some_and(|click| Some(click.button) == button)
                {
                    self.current_click = None;
                }

                let mods = Self::wparam_to_mods(wparam);
                let (x, y) = Self::lparam_to_xy(lparam);

                self.mousebinds_pressed.push(Mousebind::new(
                    button,
                    x,
                    y,
                    mods,
                    MouseClickCount::Single,
                    MouseClickKind::Release,
                ));
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_MOUSEMOVE => {
                let button = if msg == WM_MOUSEMOVE {
                    self.current_click.map(|click| click.button)
                } else if wparam.0 & MK_LBUTTON != 0 {
                    Some(MouseButton::Left)
                } else if wparam.0 & MK_MBUTTON != 0 {
                    Some(MouseButton::Middle)
                } else if wparam.0 & MK_RBUTTON != 0 {
                    Some(MouseButton::Right)
                } else if wparam.0 & MK_XBUTTON1 != 0 {
                    Some(MouseButton::FirstSide)
                } else if wparam.0 & MK_XBUTTON2 != 0 {
                    Some(MouseButton::SecondSide)
                } else {
                    None
                };

                let mods = Self::wparam_to_mods(wparam);
                let (x, y) = Self::lparam_to_xy(lparam);

                let (count, kind) = match msg {
                    WM_MOUSEMOVE => {
                        let count = self
                            .current_click
                            .map(|click| click.count)
                            .unwrap_or(MouseClickCount::Single);

                        self.last_click =
                            self.last_click.filter(|click| x == click.x && y == click.y);

                        (count, MouseClickKind::Drag)
                    }
                    _ => {
                        let (is_chained_click, previous_kind) = self
                            .last_click
                            .map(|last_click| {
                                (
                                    Some(last_click.button) == button
                                        && self.time - last_click.time <= self.double_click_time,
                                    last_click.count,
                                )
                            })
                            .unwrap_or((false, MouseClickCount::Single));

                        let count = if is_chained_click {
                            match previous_kind {
                                MouseClickCount::Single => MouseClickCount::Double,
                                MouseClickCount::Double => MouseClickCount::Triple,
                                MouseClickCount::Triple => MouseClickCount::Single,
                            }
                        } else {
                            MouseClickCount::Single
                        };

                        let click = button.map(|button| RecordedMouseClick {
                            button,
                            count,
                            x,
                            y,
                            time: self.time,
                        });

                        self.last_click = click;
                        self.current_click = click;

                        (count, MouseClickKind::Press)
                    }
                };

                let do_ignore = if let Some(button) = button {
                    if kind != MouseClickKind::Drag {
                        self.draggable_buttons.insert(button);
                    }

                    kind == MouseClickKind::Drag && !self.draggable_buttons.contains(&button)
                } else {
                    false
                };

                if !do_ignore {
                    self.mousebinds_pressed
                        .push(Mousebind::new(button, x, y, mods, count, kind));
                }
            }
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                const WHEEL_DELTA: f32 = 120.0;

                let delta =
                    transmute::<u16, i16>(((wparam.0 >> 16) & 0xFFFF) as u16) as f32 / WHEEL_DELTA;

                let is_horizontal = msg == WM_MOUSEHWHEEL;

                let (x, y) = Self::lparam_to_xy(lparam);

                self.mouse_scrolls.push(MouseScroll {
                    delta,
                    is_horizontal,
                    is_precise: false,
                    x: x - self.x as f32,
                    y: y - self.y as f32,
                });
            }
            WM_CLIPBOARDUPDATE => {
                if !self.did_just_copy {
                    self.was_copy_implicit = false;
                }

                self.did_just_copy = false;
            }
            _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
        }

        LRESULT(0)
    }

    unsafe fn lparam_to_xy(lparam: LPARAM) -> (f32, f32) {
        let x = transmute::<u32, i32>((lparam.0 & 0xFFFF) as u32) as f32;
        let y = transmute::<u32, i32>(((lparam.0 >> 16) & 0xFFFF) as u32) as f32;

        (x, y)
    }

    fn key_state_to_mods() -> Mods {
        const LSHIFT: i32 = VK_LSHIFT.0 as i32;
        const RSHIFT: i32 = VK_RSHIFT.0 as i32;
        const LCTRL: i32 = VK_LCONTROL.0 as i32;
        const RCTRL: i32 = VK_RCONTROL.0 as i32;
        const LALT: i32 = VK_LMENU.0 as i32;
        const RALT: i32 = VK_RMENU.0 as i32;

        let mut mods = Mods::NONE;

        unsafe {
            if GetKeyState(LSHIFT) < 0 || GetKeyState(RSHIFT) < 0 {
                mods = mods.with(Mod::Shift);
            }

            if GetKeyState(LCTRL) < 0 || GetKeyState(RCTRL) < 0 {
                mods = mods.with(Mod::Ctrl);
            }

            if GetKeyState(LALT) < 0 || GetKeyState(RALT) < 0 {
                mods = mods.with(Mod::Alt);
            }
        }

        mods
    }

    fn wparam_to_mods(wparam: WPARAM) -> Mods {
        let mut mods = Mods::NONE;

        if wparam.0 & MK_SHIFT != 0 {
            mods = mods.with(Mod::Shift);
        }

        if wparam.0 & MK_CONTROL != 0 {
            mods = mods.with(Mod::Ctrl);
        }

        mods
    }

    fn key_from_keycode(value: u32) -> Option<Key> {
        match value {
            0x0 => Some(Key::Null),
            0x01 => Some(Key::LButton),
            0x02 => Some(Key::RButton),
            0x03 => Some(Key::Cancel),
            0x04 => Some(Key::MButton),
            0x05 => Some(Key::XButton1),
            0x06 => Some(Key::XButton2),
            0x08 => Some(Key::Backspace),
            0x09 => Some(Key::Tab),
            0x0C => Some(Key::Clear),
            0x0D => Some(Key::Enter),
            0x10 => Some(Key::Shift),
            0x11 => Some(Key::Ctrl),
            0x12 => Some(Key::Alt),
            0x13 => Some(Key::Pause),
            0x14 => Some(Key::Capital),
            0x1B => Some(Key::Escape),
            0x1C => Some(Key::Convert),
            0x1E => Some(Key::Accept),
            0x20 => Some(Key::Space),
            0x21 => Some(Key::PageUp),
            0x22 => Some(Key::PageDown),
            0x23 => Some(Key::End),
            0x24 => Some(Key::Home),
            0x25 => Some(Key::Left),
            0x26 => Some(Key::Up),
            0x27 => Some(Key::Right),
            0x28 => Some(Key::Down),
            0x29 => Some(Key::Select),
            0x2A => Some(Key::Print),
            0x2B => Some(Key::Execute),
            0x2C => Some(Key::Snapshot),
            0x2D => Some(Key::Insert),
            0x2E => Some(Key::Delete),
            0x2F => Some(Key::Help),
            0x30 => Some(Key::Zero),
            0x31 => Some(Key::One),
            0x32 => Some(Key::Two),
            0x33 => Some(Key::Three),
            0x34 => Some(Key::Four),
            0x35 => Some(Key::Five),
            0x36 => Some(Key::Six),
            0x37 => Some(Key::Seven),
            0x38 => Some(Key::Eight),
            0x39 => Some(Key::Nine),
            0x41 => Some(Key::A),
            0x42 => Some(Key::B),
            0x43 => Some(Key::C),
            0x44 => Some(Key::D),
            0x45 => Some(Key::E),
            0x46 => Some(Key::F),
            0x47 => Some(Key::G),
            0x48 => Some(Key::H),
            0x49 => Some(Key::I),
            0x4A => Some(Key::J),
            0x4B => Some(Key::K),
            0x4C => Some(Key::L),
            0x4D => Some(Key::M),
            0x4E => Some(Key::N),
            0x4F => Some(Key::O),
            0x50 => Some(Key::P),
            0x51 => Some(Key::Q),
            0x52 => Some(Key::R),
            0x53 => Some(Key::S),
            0x54 => Some(Key::T),
            0x55 => Some(Key::U),
            0x56 => Some(Key::V),
            0x57 => Some(Key::W),
            0x58 => Some(Key::X),
            0x59 => Some(Key::Y),
            0x5A => Some(Key::Z),
            0x70 => Some(Key::F1),
            0x71 => Some(Key::F2),
            0x72 => Some(Key::F3),
            0x73 => Some(Key::F4),
            0x74 => Some(Key::F5),
            0x75 => Some(Key::F6),
            0x76 => Some(Key::F7),
            0x77 => Some(Key::F8),
            0x78 => Some(Key::F9),
            0x79 => Some(Key::F10),
            0x7A => Some(Key::F11),
            0x7B => Some(Key::F12),
            0xA0 => Some(Key::LShift),
            0xA1 => Some(Key::RShift),
            0xA2 => Some(Key::LCtrl),
            0xA3 => Some(Key::RCtrl),
            0xA4 => Some(Key::LAlt),
            0xA5 => Some(Key::RAlt),
            0xBF => Some(Key::ForwardSlash),
            0xBD => Some(Key::Minus),
            0xBE => Some(Key::Period),
            0xC0 => Some(Key::Grave),
            0xDB => Some(Key::LBracket),
            0xDD => Some(Key::RBracket),
            _ => None,
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
