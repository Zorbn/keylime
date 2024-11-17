use std::{
    collections::HashSet,
    mem::transmute,
    ptr::{copy_nonoverlapping, null_mut},
};

use windows::{
    core::{w, Result},
    Win32::{
        Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, RECT, WPARAM},
        System::{
            Com::{
                CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            },
            DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard, SetClipboardData},
            LibraryLoader::GetModuleHandleW,
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
            Ole::CF_UNICODETEXT,
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
        },
        UI::{Input::KeyboardAndMouse::GetKeyState, WindowsAndMessaging::*},
    },
};

use crate::{
    defer,
    gfx::Gfx,
    input_handlers::{CharHandler, KeybindHandler, MouseScrollHandler, MousebindHandler},
    key::Key,
    keybind::Keybind,
    mouse_button::MouseButton,
    mouse_scroll::MouseScroll,
    mousebind::Mousebind,
};

const DEFAULT_WIDTH: i32 = 640;
const DEFAULT_HEIGHT: i32 = 480;

pub struct Window {
    timer_frequency: i64,
    last_queried_time: Option<i64>,
    time: f32,

    hwnd: HWND,

    is_running: bool,
    is_focused: bool,

    width: i32,
    height: i32,

    wide_text_buffer: Vec<u16>,
    text_buffer: Vec<char>,

    gfx: Option<Gfx>,

    pub chars_typed: Vec<char>,
    pub keybinds_typed: Vec<Keybind>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,

    // Keep track of which mouse buttons have been pressed since the window was
    // last focused, so that we can skip stray mouse drag events that may happen
    // when the window is gaining focus again after coming back from a popup.
    draggable_buttons: HashSet<MouseButton>,

    was_copy_implicit: bool,
    did_just_copy: bool,
}

impl Window {
    pub fn new() -> Result<Box<Self>> {
        unsafe {
            let mut timer_frequency = 0i64;

            QueryPerformanceFrequency(&mut timer_frequency)?;

            let window_class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(Self::window_proc),
                hInstance: GetModuleHandleW(None)?.into(),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                lpszClassName: w!("keylime_window_class"),
                ..Default::default()
            };

            assert!(RegisterClassExW(&window_class) != 0);

            let mut window = Box::new(Window {
                timer_frequency,
                last_queried_time: None,
                time: 0.0,

                hwnd: HWND(null_mut()),

                is_running: true,
                is_focused: false,

                width: DEFAULT_WIDTH,
                height: DEFAULT_HEIGHT,

                wide_text_buffer: Vec::new(),
                text_buffer: Vec::new(),

                gfx: None,

                chars_typed: Vec::new(),
                keybinds_typed: Vec::new(),
                mousebinds_pressed: Vec::new(),
                mouse_scrolls: Vec::new(),

                draggable_buttons: HashSet::new(),

                was_copy_implicit: false,
                did_just_copy: false,
            });

            let lparam: *mut Window = &mut *window;

            let mut window_rect = RECT {
                left: 0,
                top: 0,
                right: 640,
                bottom: 480,
            };

            AdjustWindowRectEx(
                &mut window_rect,
                WS_OVERLAPPEDWINDOW,
                false,
                WINDOW_EX_STYLE::default(),
            )?;

            let width = window_rect.right - window_rect.left;
            let height = window_rect.bottom - window_rect.top;

            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                window_class.lpszClassName,
                w!("Keylime"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                width,
                height,
                None,
                None,
                window_class.hInstance,
                Some(lparam.cast()),
            )?;

            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?;

            Ok(window)
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let window = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Window;

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
            WM_CREATE => {
                (*window).hwnd = hwnd;
                (*window).gfx = Some(Gfx::new(window.as_ref().unwrap()).unwrap());
            }
            WM_SIZE => {
                let width = (lparam.0 & 0xffff) as i32;
                let height = ((lparam.0 >> 16) & 0xffff) as i32;

                (*window).width = width;
                (*window).height = height;

                if let Some(gfx) = &mut (*window).gfx {
                    gfx.resize(width, height).unwrap();
                }
            }
            WM_CLOSE => {
                (*window).is_running = false;
            }
            WM_DESTROY => {
                PostQuitMessage(0);
            }
            WM_SETFOCUS => {
                (*window).is_focused = true;
            }
            WM_KILLFOCUS => {
                (*window).is_focused = false;
                (*window).draggable_buttons.clear();

                let _ = PostMessageW((*window).hwnd, WM_PAINT, None, None);
            }
            WM_CHAR => {
                if let Some(char) = char::from_u32(wparam.0 as u32) {
                    if !char.is_control() {
                        (*window).chars_typed.push(char);
                    }
                }
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if let Some(key) = Key::from((wparam.0 & 0xffff) as u32) {
                    let has_shift =
                        GetKeyState(Key::LShift as i32) < 0 || GetKeyState(Key::RShift as i32) < 0;

                    let has_ctrl =
                        GetKeyState(Key::LCtrl as i32) < 0 || GetKeyState(Key::RCtrl as i32) < 0;

                    let has_alt =
                        GetKeyState(Key::LAlt as i32) < 0 || GetKeyState(Key::RAlt as i32) < 0;

                    (*window)
                        .keybinds_typed
                        .push(Keybind::new(key, has_shift, has_ctrl, has_alt));
                }

                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_MOUSEMOVE => {
                const MK_LBUTTON: usize = 0x01;
                const MK_RBUTTON: usize = 0x02;
                const MK_MBUTTON: usize = 0x10;
                const MK_XBUTTON1: usize = 0x20;
                const MK_XBUTTON2: usize = 0x40;
                const MK_SHIFT: usize = 0x04;
                const MK_CONTROL: usize = 0x08;

                let button = if wparam.0 & MK_LBUTTON != 0 {
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

                if let Some(button) = button {
                    let has_shift = wparam.0 & MK_SHIFT != 0;
                    let has_ctrl = wparam.0 & MK_CONTROL != 0;

                    let x = transmute::<u32, i32>((lparam.0 & 0xffff) as u32) as f32;
                    let y = transmute::<u32, i32>(((lparam.0 >> 16) & 0xffff) as u32) as f32;

                    let is_drag = msg == WM_MOUSEMOVE;

                    if !is_drag || (*window).draggable_buttons.contains(&button) {
                        (*window).draggable_buttons.insert(button);
                        (*window).mousebinds_pressed.push(Mousebind::new(
                            button, x, y, has_shift, has_ctrl, false, is_drag,
                        ));
                    }
                }
            }
            WM_MOUSEWHEEL => {
                const WHEEL_DELTA: f32 = 120.0;

                let delta =
                    transmute::<u16, i16>(((wparam.0 >> 16) & 0xffff) as u16) as f32 / WHEEL_DELTA;

                (*window).mouse_scrolls.push(MouseScroll { delta });
            }
            WM_CLIPBOARDUPDATE => {
                if !(*window).did_just_copy {
                    (*window).was_copy_implicit = false;
                }

                (*window).did_just_copy = false;
            }
            _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
        }

        LRESULT(0)
    }

    pub fn update(&mut self, is_animating: bool) -> (f32, f32) {
        self.chars_typed.clear();
        self.keybinds_typed.clear();
        self.mousebinds_pressed.clear();
        self.mouse_scrolls.clear();

        unsafe {
            let mut msg = MSG::default();

            if !is_animating {
                let _ = GetMessageW(&mut msg, self.hwnd, 0, 0);
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            };

            while PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

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

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn gfx(&mut self) -> &mut Gfx {
        self.gfx.as_mut().unwrap()
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
        self.was_copy_implicit = was_copy_implicit;
        self.did_just_copy = true;

        self.wide_text_buffer.clear();

        for c in text {
            let mut dst = [0u16; 2];

            for wide_c in c.encode_utf16(&mut dst) {
                self.wide_text_buffer.push(*wide_c);
            }
        }

        self.wide_text_buffer.push(0);

        unsafe {
            OpenClipboard(self.hwnd)?;

            defer!({
                let _ = CloseClipboard();
            });

            let hglobal = GlobalAlloc(
                GMEM_MOVEABLE,
                self.wide_text_buffer.len() * size_of::<u16>(),
            )?;

            defer!({
                let _ = GlobalFree(hglobal);
            });

            let memory = GlobalLock(hglobal) as *mut u16;

            copy_nonoverlapping(
                self.wide_text_buffer.as_ptr(),
                memory,
                self.wide_text_buffer.len(),
            );

            let _ = GlobalUnlock(hglobal);

            SetClipboardData(CF_UNICODETEXT.0 as u32, HANDLE(hglobal.0))?;
        }

        Ok(())
    }

    pub fn get_clipboard(&mut self) -> Result<&[char]> {
        self.text_buffer.clear();
        self.wide_text_buffer.clear();

        unsafe {
            OpenClipboard(self.hwnd)?;

            defer!({
                let _ = CloseClipboard();
            });

            let hglobal = GlobalAlloc(
                GMEM_MOVEABLE,
                self.wide_text_buffer.len() * size_of::<u16>(),
            )?;

            defer!({
                let _ = GlobalFree(hglobal);
            });

            let hglobal = HGLOBAL(GetClipboardData(CF_UNICODETEXT.0 as u32)?.0);

            let mut memory = GlobalLock(hglobal) as *mut u16;

            if !memory.is_null() {
                while *memory != 0 {
                    self.wide_text_buffer.push(*memory);
                    memory = memory.add(1);
                }
            }

            let _ = GlobalUnlock(hglobal);
        }

        for c in char::decode_utf16(self.wide_text_buffer.iter().copied()) {
            let c = c.unwrap_or('?');

            if c == '\r' {
                continue;
            }

            self.text_buffer.push(c);
        }

        Ok(&self.text_buffer)
    }

    pub fn was_copy_implicit(&self) -> bool {
        self.was_copy_implicit
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
            CoUninitialize();
        }
    }
}
