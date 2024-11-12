use std::{char, ptr::null_mut};

use windows::{
    core::{w, Result},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
        },
        UI::{Input::KeyboardAndMouse::GetKeyState, WindowsAndMessaging::*},
    },
};

use crate::{gfx::Gfx, key::Key, keybind::Keybind};

const DEFAULT_WIDTH: i32 = 640;
const DEFAULT_HEIGHT: i32 = 480;

pub struct Window {
    timer_frequency: i64,
    last_time: i64,
    hwnd: HWND,
    is_running: bool,
    is_focused: bool,
    width: i32,
    height: i32,

    gfx: Option<Gfx>,

    chars_typed: Vec<char>,
    keybinds_typed: Vec<Keybind>,
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
                last_time: 0,
                hwnd: HWND(null_mut()),
                is_running: true,
                is_focused: false,
                width: DEFAULT_WIDTH,
                height: DEFAULT_HEIGHT,

                gfx: None,

                chars_typed: Vec::new(),
                keybinds_typed: Vec::new(),
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
            _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
        }

        LRESULT(0)
    }

    pub fn update(&mut self) -> f32 {
        self.chars_typed.clear();
        self.keybinds_typed.clear();

        unsafe {
            let mut msg = MSG::default();

            let _ = GetMessageW(&mut msg, self.hwnd, 0, 0);
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);

            let mut time = 0i64;
            let _ = QueryPerformanceCounter(&mut time);

            let dt = (time - self.last_time) as f32 / self.timer_frequency as f32;
            self.last_time = time;

            dt
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn gfx(&mut self) -> &mut Gfx {
        self.gfx.as_mut().unwrap()
    }

    pub fn get_typed_char(&mut self) -> Option<char> {
        self.chars_typed.pop()
    }

    pub fn get_typed_keybind(&mut self) -> Option<Keybind> {
        self.keybinds_typed.pop()
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
