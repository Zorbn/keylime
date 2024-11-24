use std::{
    collections::HashSet,
    mem::transmute,
    ptr::{copy_nonoverlapping, null_mut},
};

use windows::{
    core::{w, Result},
    Win32::{
        Foundation::{GlobalFree, BOOL, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE},
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
        UI::{
            HiDpi::GetDpiForWindow, Input::KeyboardAndMouse::GetKeyState, WindowsAndMessaging::*,
        },
    },
};

use crate::{
    app::App,
    config::Config,
    defer,
    input::{
        input_handlers::{CharHandler, KeybindHandler, MouseScrollHandler, MousebindHandler},
        key::Key,
        keybind::Keybind,
        mouse_button::MouseButton,
        mouse_scroll::MouseScroll,
        mousebind::Mousebind,
    },
};

use super::gfx::Gfx;

const DEFAULT_DPI: f32 = 96.0;

pub struct WindowRunner {
    window: Window,
    app: App,
}

impl WindowRunner {
    pub fn new(app: App) -> Result<Box<Self>> {
        let use_dark_mode = BOOL::from(app.is_dark());

        let mut window_runner = Box::new(WindowRunner {
            window: Window::new()?,
            app,
        });

        unsafe {
            let window_class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(Self::window_proc),
                hInstance: GetModuleHandleW(None)?.into(),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                lpszClassName: w!("keylime_window_class"),
                ..Default::default()
            };

            assert!(RegisterClassExW(&window_class) != 0);

            let lparam: *mut WindowRunner = &mut *window_runner;

            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                window_class.lpszClassName,
                w!("Keylime"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                0,
                0,
                None,
                None,
                window_class.hInstance,
                Some(lparam.cast()),
            )?;

            DwmSetWindowAttribute(
                window_runner.window.hwnd(),
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &use_dark_mode as *const BOOL as _,
                size_of::<BOOL>() as u32,
            )?;

            let _ = ShowWindow(window_runner.window.hwnd(), SW_SHOWDEFAULT);

            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?;
        }

        Ok(window_runner)
    }

    pub fn run(&mut self) {
        let WindowRunner {
            window: window_handle,
            app,
            ..
        } = self;

        while window_handle.is_running() {
            app.update(window_handle);
            app.draw(window_handle);
        }

        app.close(self.window.time);
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let window_runner = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowRunner;

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

                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => {
                let WindowRunner {
                    window: window_handle,
                    app,
                    ..
                } = &mut (*window_runner);

                window_handle.window_proc(app, hwnd, msg, wparam, lparam)
            }
        }
    }
}

impl Drop for WindowRunner {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window.hwnd());
            CoUninitialize();
        }
    }
}

const DEFAULT_WIDTH: i32 = 640;
const DEFAULT_HEIGHT: i32 = 480;

pub struct Window {
    timer_frequency: i64,
    last_queried_time: Option<i64>,
    time: f32,

    hwnd: HWND,

    is_running: bool,
    is_focused: bool,

    dpi: f32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,

    wide_text_buffer: Vec<u16>,
    text_buffer: Vec<char>,

    gfx: Option<Gfx>,

    // Keep track of which mouse buttons have been pressed since the window was
    // last focused, so that we can skip stray mouse drag events that may happen
    // when the window is gaining focus again after coming back from a popup.
    draggable_buttons: HashSet<MouseButton>,

    pub chars_typed: Vec<char>,
    pub keybinds_typed: Vec<Keybind>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,

    was_copy_implicit: bool,
    did_just_copy: bool,
}

impl Window {
    fn new() -> Result<Self> {
        let mut timer_frequency = 0i64;

        unsafe {
            QueryPerformanceFrequency(&mut timer_frequency)?;
        }

        Ok(Self {
            timer_frequency,
            last_queried_time: None,
            time: 0.0,

            hwnd: HWND(null_mut()),

            is_running: true,
            is_focused: false,

            dpi: 1.0,
            x: 0,
            y: 0,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,

            wide_text_buffer: Vec::new(),
            text_buffer: Vec::new(),

            gfx: None,

            draggable_buttons: HashSet::new(),

            chars_typed: Vec::new(),
            keybinds_typed: Vec::new(),
            mousebinds_pressed: Vec::new(),
            mouse_scrolls: Vec::new(),

            was_copy_implicit: false,
            did_just_copy: false,
        })
    }

    pub fn clear_inputs(&mut self) {
        self.chars_typed.clear();
        self.keybinds_typed.clear();
        self.mousebinds_pressed.clear();
        self.mouse_scrolls.clear();
    }

    pub fn update(&mut self, is_animating: bool) -> (f32, f32) {
        self.clear_inputs();

        unsafe {
            let mut msg = MSG::default();

            if !is_animating {
                let _ = GetMessageW(&mut msg, self.hwnd, 0, 0);
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

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

    pub fn dpi(&self) -> f32 {
        self.dpi
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

    unsafe fn window_proc(
        &mut self,
        app: &mut App,
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
            WM_CREATE => {
                self.hwnd = hwnd;
                self.dpi = GetDpiForWindow(hwnd) as f32 / DEFAULT_DPI;

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

                let width = ((window_rect.right - window_rect.left) as f32 * self.dpi) as i32;
                let height = ((window_rect.bottom - window_rect.top) as f32 * self.dpi) as i32;

                let _ = SetWindowPos(hwnd, None, 0, 0, width, height, SWP_NOMOVE);

                let Config {
                    font, font_size, ..
                } = app.config();

                self.gfx = Some(Gfx::new(font, *font_size, self).unwrap());
            }
            WM_DPICHANGED => {
                let dpi = (wparam.0 & 0xffff) as f32 / DEFAULT_DPI;
                self.dpi = dpi;

                if let Some(gfx) = &mut self.gfx {
                    let Config {
                        font, font_size, ..
                    } = app.config();

                    gfx.update_font(font, *font_size, dpi);
                }

                let rect = *(lparam.0 as *const RECT);

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
            WM_MOVE => {
                self.x = transmute::<u32, i32>((lparam.0 & 0xffff) as u32);
                self.y = transmute::<u32, i32>(((lparam.0 >> 16) & 0xffff) as u32);
            }
            WM_SIZE => {
                let width = (lparam.0 & 0xffff) as i32;
                let height = ((lparam.0 >> 16) & 0xffff) as i32;

                self.width = width;
                self.height = height;

                if let Some(gfx) = &mut self.gfx {
                    gfx.resize(width, height).unwrap();
                    app.draw(self);
                }
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

                let _ = PostMessageW(self.hwnd, WM_PAINT, None, None);
            }
            WM_CHAR => {
                if let Some(char) = char::from_u32(wparam.0 as u32) {
                    if !char.is_control() {
                        self.chars_typed.push(char);
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

                    self.keybinds_typed
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

                let has_shift = wparam.0 & MK_SHIFT != 0;
                let has_ctrl = wparam.0 & MK_CONTROL != 0;

                let x = transmute::<u32, i32>((lparam.0 & 0xffff) as u32) as f32;
                let y = transmute::<u32, i32>(((lparam.0 >> 16) & 0xffff) as u32) as f32;

                let is_drag = msg == WM_MOUSEMOVE;

                let do_ignore = if let Some(button) = button {
                    if !is_drag {
                        self.draggable_buttons.insert(button);
                    }

                    is_drag && !self.draggable_buttons.contains(&button)
                } else {
                    false
                };

                if !do_ignore {
                    self.mousebinds_pressed.push(Mousebind::new(
                        button, x, y, has_shift, has_ctrl, false, is_drag,
                    ));
                }
            }
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                const WHEEL_DELTA: f32 = 120.0;

                let delta =
                    transmute::<u16, i16>(((wparam.0 >> 16) & 0xffff) as u16) as f32 / WHEEL_DELTA;

                let is_horizontal = msg == WM_MOUSEHWHEEL;

                let x = transmute::<u32, i32>((lparam.0 & 0xffff) as u32);
                let y = transmute::<u32, i32>(((lparam.0 >> 16) & 0xffff) as u32);

                self.mouse_scrolls.push(MouseScroll {
                    delta,
                    is_horizontal,
                    x: (x - self.x) as f32,
                    y: (y - self.y) as f32,
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
}
