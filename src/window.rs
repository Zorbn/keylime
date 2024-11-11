use std::ptr::null_mut;

use windows::{
    core::{w, Result},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
        },
        UI::WindowsAndMessaging::*,
    },
};

use crate::gfx::Gfx;

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
            });

            let lparam: *mut Window = &mut *window;

            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                window_class.lpszClassName,
                w!("Keylime"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                window.width,
                window.height,
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
                (*window).gfx = Gfx::new(window.as_ref().unwrap()).ok();
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
            _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
        }

        LRESULT(0)
    }

    pub fn update(&mut self) -> f32 {
        unsafe {
            let mut msg = MSG::default();

            let mut time = 0i64;
            let _ = QueryPerformanceCounter(&mut time);

            let dt = (time - self.last_time) as f32 / self.timer_frequency as f32;
            self.last_time = time;

            while PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

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
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
