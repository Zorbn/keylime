use std::mem::ManuallyDrop;

use windows::{
    core::{w, Result},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        System::{
            Com::{
                CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            },
            DataExchange::AddClipboardFormatListener,
            LibraryLoader::GetModuleHandleW,
        },
        UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    },
};

use crate::{
    app::App,
    config::Config,
    platform::aliases::{AnyGfx, AnyWindow},
};

use super::{gfx::Gfx, window::Window};

const DEFAULT_DPI: f32 = 96.0;

pub struct AppRunner {
    window: ManuallyDrop<AnyWindow>,
    gfx: Option<AnyGfx>,
    app: ManuallyDrop<App>,
}

impl AppRunner {
    fn new() -> Result<Self> {
        Ok(Self {
            window: ManuallyDrop::new(AnyWindow {
                inner: Window::new()?,
            }),
            gfx: None,
            app: ManuallyDrop::new(App::new()),
        })
    }

    unsafe fn create_window(&mut self) -> Result<()> {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?;

        let window_class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(Self::window_proc),
            hInstance: GetModuleHandleW(None)?.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: w!("keylime_window_class"),
            ..Default::default()
        };

        assert!(RegisterClassExW(&window_class) != 0);

        let lparam: *mut Self = self;

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
            Some(window_class.hInstance),
            Some(lparam.cast()),
        )?;

        self.window.set_theme(&self.app.config().theme);

        AddClipboardFormatListener(self.window.inner.hwnd())?;

        let _ = ShowWindow(self.window.inner.hwnd(), SW_SHOWDEFAULT);

        Ok(())
    }

    pub fn run(&mut self) {
        let AppRunner {
            window, gfx, app, ..
        } = self;

        while window.inner.is_running() {
            let is_animating = app.is_animating();

            let (file_watcher, files, processes) = app.files_and_processes();
            window
                .inner
                .update(is_animating, file_watcher, files, processes);

            let Some(gfx) = gfx else {
                continue;
            };

            let (time, dt) = window.inner.get_time(is_animating);

            app.update(window, gfx, time, dt);
            app.draw(window, gfx, time);
        }

        let time = window.inner.time;

        if let Some(gfx) = gfx {
            app.close(window, gfx, time);
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let app_runner = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppRunner;

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
            WM_CREATE => {
                let app_runner = &mut *app_runner;

                let scale = GetDpiForWindow(hwnd) as f32 / DEFAULT_DPI;

                app_runner.window.inner.on_create(scale, hwnd);

                let Config {
                    font, font_size, ..
                } = app_runner.app.config();

                app_runner.gfx = Some(AnyGfx {
                    inner: Gfx::new(font, *font_size, scale, hwnd).unwrap(),
                });

                LRESULT(0)
            }
            WM_DPICHANGED => {
                let app_runner = &mut *app_runner;

                let scale = (wparam.0 & 0xFFFF) as f32 / DEFAULT_DPI;
                let rect = *(lparam.0 as *const RECT);

                app_runner.window.inner.on_dpi_changed(rect);

                if let Some(gfx) = &mut app_runner.gfx {
                    let Config {
                        font, font_size, ..
                    } = app_runner.app.config();

                    gfx.inner.update_font(font, *font_size, scale);
                }

                LRESULT(0)
            }
            WM_SIZE => {
                let width = (lparam.0 & 0xFFFF) as i32;
                let height = ((lparam.0 >> 16) & 0xFFFF) as i32;

                let app_runner = &mut *app_runner;

                app_runner.window.inner.width = width;
                app_runner.window.inner.height = height;

                if let AppRunner {
                    window,
                    gfx: Some(gfx),
                    app,
                    ..
                } = app_runner
                {
                    let time = window.inner.time;

                    gfx.inner.resize(width, height).unwrap();
                    app.draw(window, gfx, time);
                }

                LRESULT(0)
            }
            _ => {
                let app_runner = &mut *app_runner;

                app_runner
                    .window
                    .inner
                    .window_proc(hwnd, msg, wparam, lparam)
            }
        }
    }
}

impl Drop for AppRunner {
    fn drop(&mut self) {
        unsafe {
            self.gfx.take();
            ManuallyDrop::drop(&mut self.window);
            ManuallyDrop::drop(&mut self.app);
            CoUninitialize();
        }
    }
}

pub fn run_app() -> Result<()> {
    let mut app_runner = AppRunner::new()?;

    unsafe {
        app_runner.create_window()?;
    }

    app_runner.run();

    Ok(())
}
