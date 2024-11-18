use std::{collections::HashSet, mem::transmute, ptr::null_mut};

use windows::{
    core::{w, Result},
    Win32::{
        Foundation::{BOOL, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE},
        System::{
            Com::{
                CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            },
            LibraryLoader::GetModuleHandleW,
            Performance::QueryPerformanceFrequency,
        },
        UI::{
            HiDpi::GetDpiForWindow, Input::KeyboardAndMouse::GetKeyState, WindowsAndMessaging::*,
        },
    },
};

use crate::{
    app::App, gfx::Gfx, key::Key, keybind::Keybind, mouse_button::MouseButton,
    mouse_scroll::MouseScroll, mousebind::Mousebind, window_handle::WindowHandle,
};

const DEFAULT_DPI: f32 = 92.0;
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

    app: App,
}

impl Window {
    pub fn new(app: App, use_dark_mode: bool) -> Result<Box<Self>> {
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

                dpi: 1.0,
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

                app,
            });

            let lparam: *mut Window = &mut *window;

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

            let use_dark_mode = BOOL::from(use_dark_mode);

            DwmSetWindowAttribute(
                window.hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &use_dark_mode as *const BOOL as _,
                size_of::<BOOL>() as u32,
            )?;

            let _ = ShowWindow(window.hwnd, SW_SHOWDEFAULT);

            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?;

            Ok(window)
        }
    }

    fn as_window_handle(&mut self) -> (WindowHandle, &mut App) {
        let Window {
            timer_frequency,
            last_queried_time,
            time,
            hwnd,
            is_running,
            is_focused,
            dpi,
            width,
            height,
            wide_text_buffer,
            text_buffer,
            gfx,
            chars_typed,
            keybinds_typed,
            mousebinds_pressed,
            mouse_scrolls,
            was_copy_implicit,
            did_just_copy,
            app,
            ..
        } = self;

        let window_handle = WindowHandle::new(
            timer_frequency,
            last_queried_time,
            time,
            hwnd,
            is_running,
            is_focused,
            dpi,
            width,
            height,
            wide_text_buffer,
            text_buffer,
            gfx,
            chars_typed,
            keybinds_typed,
            mousebinds_pressed,
            mouse_scrolls,
            was_copy_implicit,
            did_just_copy,
        );

        (window_handle, app)
    }

    pub fn run(&mut self) {
        let (mut window_handle, app) = self.as_window_handle();

        while window_handle.is_running() {
            app.update(&mut window_handle);
            app.draw(&mut window_handle);
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
                (*window).dpi = GetDpiForWindow(hwnd) as f32 / DEFAULT_DPI;

                let mut window_rect = RECT {
                    left: 0,
                    top: 0,
                    right: (*window).width,
                    bottom: (*window).height,
                };

                AdjustWindowRectEx(
                    &mut window_rect,
                    WS_OVERLAPPEDWINDOW,
                    false,
                    WINDOW_EX_STYLE::default(),
                )
                .unwrap();

                let width = ((window_rect.right - window_rect.left) as f32 * (*window).dpi) as i32;
                let height = ((window_rect.bottom - window_rect.top) as f32 * (*window).dpi) as i32;

                let _ = SetWindowPos(hwnd, None, 0, 0, width, height, SWP_NOMOVE);

                let (window_handle, _) = (*window).as_window_handle();
                (*window).gfx = Some(Gfx::new(&window_handle).unwrap());
            }
            WM_DPICHANGED => {
                let dpi = (wparam.0 & 0xffff) as f32 / DEFAULT_DPI;
                (*window).dpi = dpi;

                if let Some(gfx) = &mut (*window).gfx {
                    gfx.set_scale(dpi);
                }

                let rect = *(lparam.0 as *const RECT);

                let _ = SetWindowPos(
                    (*window).hwnd,
                    None,
                    0,
                    0,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE,
                );
            }
            WM_SIZE => {
                let width = (lparam.0 & 0xffff) as i32;
                let height = ((lparam.0 >> 16) & 0xffff) as i32;

                (*window).width = width;
                (*window).height = height;

                if let Some(gfx) = &mut (*window).gfx {
                    gfx.resize(width, height).unwrap();

                    let (mut window_handle, app) = (*window).as_window_handle();
                    app.draw(&mut window_handle);
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

                let has_shift = wparam.0 & MK_SHIFT != 0;
                let has_ctrl = wparam.0 & MK_CONTROL != 0;

                let x = transmute::<u32, i32>((lparam.0 & 0xffff) as u32) as f32;
                let y = transmute::<u32, i32>(((lparam.0 >> 16) & 0xffff) as u32) as f32;

                let is_drag = msg == WM_MOUSEMOVE;

                let do_ignore = if let Some(button) = button {
                    if !is_drag {
                        (*window).draggable_buttons.insert(button);
                    }

                    is_drag && !(*window).draggable_buttons.contains(&button)
                } else {
                    false
                };

                if !do_ignore {
                    (*window).mousebinds_pressed.push(Mousebind::new(
                        button, x, y, has_shift, has_ctrl, false, is_drag,
                    ));
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
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
            CoUninitialize();
        }
    }
}
