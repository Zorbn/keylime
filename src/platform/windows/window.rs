use std::{
    collections::{HashMap, HashSet},
    mem::{transmute, ManuallyDrop},
    path::Path,
    ptr::{copy_nonoverlapping, null_mut},
};

use windows::{
    core::{w, Result},
    Win32::{
        Foundation::{
            GlobalFree, BOOL, FALSE, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, RECT, WAIT_OBJECT_0,
            WPARAM,
        },
        Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE},
        System::{
            Com::{
                CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            },
            DataExchange::{
                AddClipboardFormatListener, CloseClipboard, GetClipboardData, OpenClipboard,
                SetClipboardData,
            },
            LibraryLoader::GetModuleHandleW,
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
            Ole::CF_UNICODETEXT,
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
            Threading::INFINITE,
        },
        UI::{
            HiDpi::GetDpiForWindow,
            Input::KeyboardAndMouse::{
                GetDoubleClickTime, GetKeyState, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_RCONTROL,
                VK_RMENU, VK_RSHIFT,
            },
            WindowsAndMessaging::*,
        },
    },
};

use crate::{
    app::App,
    config::Config,
    input::{
        action::{Action, ActionName},
        input_handlers::{ActionHandler, CharHandler, MouseScrollHandler, MousebindHandler},
        key::Key,
        keybind::Keybind,
        mouse_button::MouseButton,
        mouse_scroll::MouseScroll,
        mousebind::{MouseClickKind, Mousebind},
    },
    platform::aliases::{AnyFileWatcher, AnyGfx, AnyPty, AnyWindow},
    temp_buffer::TempBuffer,
};

use super::{deferred_call::defer, file_watcher::FileWatcher, gfx::Gfx, keymaps::new_keymaps};

const DEFAULT_DPI: f32 = 96.0;
const DEFAULT_WIDTH: i32 = 640;
const DEFAULT_HEIGHT: i32 = 480;

pub struct WindowRunner {
    window: ManuallyDrop<AnyWindow>,
    app: ManuallyDrop<App>,
}

impl WindowRunner {
    pub fn new(app: App) -> Result<Box<Self>> {
        let use_dark_mode = BOOL::from(app.is_dark());

        let mut window_runner = Box::new(WindowRunner {
            window: ManuallyDrop::new(AnyWindow {
                inner: Window::new()?,
            }),
            app: ManuallyDrop::new(app),
        });

        unsafe {
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
                window_runner.window.inner.hwnd(),
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &use_dark_mode as *const BOOL as _,
                size_of::<BOOL>() as u32,
            )?;

            AddClipboardFormatListener(window_runner.window.inner.hwnd())?;

            let _ = ShowWindow(window_runner.window.inner.hwnd(), SW_SHOWDEFAULT);
        }

        Ok(window_runner)
    }

    pub fn run(&mut self) {
        let WindowRunner { window, app, .. } = self;

        while window.inner.is_running() {
            let is_animating = app.is_animating();

            let (files, ptys) = app.files_and_ptys();
            window.inner.update(is_animating, files, ptys);

            let (time, dt) = window.inner.get_time(is_animating);
            app.update(window, time, dt);

            app.draw(window);
        }

        app.close(self.window.inner.time);
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
            WM_SIZE => {
                let width = (lparam.0 & 0xFFFF) as i32;
                let height = ((lparam.0 >> 16) & 0xFFFF) as i32;

                let window_runner = &mut *window_runner;

                window_runner.window.inner.width = width;
                window_runner.window.inner.height = height;

                if let Some(gfx) = &mut window_runner.window.inner.gfx {
                    gfx.inner.resize(width, height).unwrap();

                    let WindowRunner { window, app, .. } = window_runner;

                    app.draw(window);
                }

                LRESULT(0)
            }
            _ => {
                let WindowRunner { window, app, .. } = &mut *window_runner;

                window.inner.window_proc(app, hwnd, msg, wparam, lparam)
            }
        }
    }
}

impl Drop for WindowRunner {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.window);
            ManuallyDrop::drop(&mut self.app);
            CoUninitialize();
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RecordedMouseClick {
    button: MouseButton,
    kind: MouseClickKind,
    x: f32,
    y: f32,
    time: f32,
}

pub struct Window {
    timer_frequency: i64,
    last_queried_time: Option<i64>,
    time: f32,

    hwnd: HWND,

    wait_handles: Vec<HANDLE>,
    file_watcher: AnyFileWatcher,

    is_running: bool,
    is_focused: bool,

    scale: f32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,

    wide_text_buffer: TempBuffer<u16>,
    text_buffer: TempBuffer<char>,

    gfx: Option<AnyGfx>,

    // Keep track of which mouse buttons have been pressed since the window was
    // last focused, so that we can skip stray mouse drag events that may happen
    // when the window is gaining focus again after coming back from a popup.
    draggable_buttons: HashSet<MouseButton>,
    current_click: Option<RecordedMouseClick>,
    last_click: Option<RecordedMouseClick>,
    double_click_time: f32,

    keymaps: HashMap<Keybind, ActionName>,
    pub chars_typed: Vec<char>,
    pub actions_typed: Vec<Action>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,

    was_copy_implicit: bool,
    did_just_copy: bool,
}

impl Window {
    fn new() -> Result<Self> {
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
            file_watcher: AnyFileWatcher {
                inner: FileWatcher::new(),
            },

            is_running: true,
            is_focused: false,

            scale: 1.0,
            x: 0,
            y: 0,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,

            wide_text_buffer: TempBuffer::new(),
            text_buffer: TempBuffer::new(),

            gfx: None,

            draggable_buttons: HashSet::new(),
            current_click: None,
            last_click: None,
            double_click_time: triple_click_time,

            keymaps: new_keymaps(),
            chars_typed: Vec::new(),
            actions_typed: Vec::new(),
            mousebinds_pressed: Vec::new(),
            mouse_scrolls: Vec::new(),

            was_copy_implicit: false,
            did_just_copy: false,
        })
    }

    fn clear_inputs(&mut self) {
        self.chars_typed.clear();
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
        files: impl Iterator<Item = &'a Path>,
        ptys: impl Iterator<Item = &'a mut AnyPty>,
    ) {
        self.clear_inputs();
        self.file_watcher.inner.update(files).unwrap();

        unsafe {
            let mut msg = MSG::default();

            if !is_animating {
                self.wait_handles.clear();

                for pty in ptys {
                    self.wait_handles
                        .extend_from_slice(&[pty.inner.hprocess, pty.inner.event]);
                }

                let dir_handles_start = self.wait_handles.len();

                self.wait_handles.extend(
                    self.file_watcher
                        .inner
                        .dir_watch_handles()
                        .iter()
                        .map(|handles| handles.event()),
                );

                let result = MsgWaitForMultipleObjects(
                    Some(&self.wait_handles),
                    FALSE,
                    INFINITE,
                    QS_ALLINPUT,
                );

                let index = (result.0 - WAIT_OBJECT_0.0) as usize;

                if index >= dir_handles_start && index < self.wait_handles.len() {
                    self.file_watcher
                        .inner
                        .handle_dir_update(index - dir_handles_start)
                        .unwrap();
                }
            }

            self.file_watcher.inner.check_dir_updates().unwrap();

            while PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE).as_bool() {
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

    pub fn get_action_handler(&self) -> ActionHandler {
        ActionHandler::new(self.actions_typed.len())
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

        let wide_text_buffer = self.wide_text_buffer.get_mut();

        for c in text {
            if !AnyWindow::is_char_copy_pastable(*c) {
                continue;
            }

            let mut dst = [0u16; 2];

            for wide_c in c.encode_utf16(&mut dst) {
                wide_text_buffer.push(*wide_c);
            }
        }

        wide_text_buffer.push(0);

        unsafe {
            OpenClipboard(self.hwnd)?;

            defer!({
                let _ = CloseClipboard();
            });

            let hglobal = GlobalAlloc(GMEM_MOVEABLE, wide_text_buffer.len() * size_of::<u16>())?;

            defer!({
                let _ = GlobalFree(hglobal);
            });

            let memory = GlobalLock(hglobal) as *mut u16;

            copy_nonoverlapping(wide_text_buffer.as_ptr(), memory, wide_text_buffer.len());

            let _ = GlobalUnlock(hglobal);

            SetClipboardData(CF_UNICODETEXT.0 as u32, HANDLE(hglobal.0))?;
        }

        Ok(())
    }

    pub fn get_clipboard(&mut self) -> Result<&[char]> {
        let text_buffer = self.text_buffer.get_mut();
        let wide_text_buffer = self.wide_text_buffer.get_mut();

        unsafe {
            OpenClipboard(self.hwnd)?;

            defer!({
                let _ = CloseClipboard();
            });

            let hglobal = GlobalAlloc(GMEM_MOVEABLE, wide_text_buffer.len() * size_of::<u16>())?;

            defer!({
                let _ = GlobalFree(hglobal);
            });

            let hglobal = HGLOBAL(GetClipboardData(CF_UNICODETEXT.0 as u32)?.0);

            let mut memory = GlobalLock(hglobal) as *mut u16;

            if !memory.is_null() {
                while *memory != 0 {
                    wide_text_buffer.push(*memory);
                    memory = memory.add(1);
                }
            }

            let _ = GlobalUnlock(hglobal);
        }

        for c in char::decode_utf16(wide_text_buffer.iter().copied()) {
            let c = c.unwrap_or('?');

            if !AnyWindow::is_char_copy_pastable(c) {
                continue;
            }

            text_buffer.push(c);
        }

        Ok(text_buffer)
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
                self.scale = GetDpiForWindow(hwnd) as f32 / DEFAULT_DPI;

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

                let width = ((window_rect.right - window_rect.left) as f32 * self.scale) as i32;
                let height = ((window_rect.bottom - window_rect.top) as f32 * self.scale) as i32;

                let _ = SetWindowPos(hwnd, None, 0, 0, width, height, SWP_NOMOVE);

                let Config {
                    font, font_size, ..
                } = app.config();

                self.gfx = Some(AnyGfx {
                    inner: Gfx::new(font, *font_size, self).unwrap(),
                });
            }
            WM_DPICHANGED => {
                let scale = (wparam.0 & 0xFFFF) as f32 / DEFAULT_DPI;
                self.scale = scale;

                if let Some(gfx) = &mut self.gfx {
                    let Config {
                        font, font_size, ..
                    } = app.config();

                    gfx.inner.update_font(font, *font_size, scale);
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

                let _ = PostMessageW(self.hwnd, WM_PAINT, None, None);
            }
            WM_CHAR => {
                if let Some(c) = char::from_u32(wparam.0 as u32) {
                    if !c.is_control() {
                        self.chars_typed.push(c);
                    }
                }
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if let Some(key) = Self::key_from_keycode((wparam.0 & 0xFFFF) as u32) {
                    const LSHIFT: i32 = VK_LSHIFT.0 as i32;
                    const RSHIFT: i32 = VK_RSHIFT.0 as i32;
                    const LCTRL: i32 = VK_LCONTROL.0 as i32;
                    const RCTRL: i32 = VK_RCONTROL.0 as i32;
                    const LALT: i32 = VK_LMENU.0 as i32;
                    const RALT: i32 = VK_RMENU.0 as i32;

                    let has_shift = GetKeyState(LSHIFT) < 0 || GetKeyState(RSHIFT) < 0;
                    let has_ctrl = GetKeyState(LCTRL) < 0 || GetKeyState(RCTRL) < 0;
                    let has_alt = GetKeyState(LALT) < 0 || GetKeyState(RALT) < 0;

                    let action = Action::from_keybind(
                        Keybind::new(key, has_shift, has_ctrl, has_alt, false),
                        &self.keymaps,
                    );

                    self.actions_typed.push(action);
                }

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
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_MOUSEMOVE => {
                const MK_LBUTTON: usize = 0x01;
                const MK_RBUTTON: usize = 0x02;
                const MK_MBUTTON: usize = 0x10;
                const MK_XBUTTON1: usize = 0x20;
                const MK_XBUTTON2: usize = 0x40;
                const MK_SHIFT: usize = 0x04;
                const MK_CONTROL: usize = 0x08;

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

                let has_shift = wparam.0 & MK_SHIFT != 0;
                let has_ctrl = wparam.0 & MK_CONTROL != 0;

                let x = transmute::<u32, i32>((lparam.0 & 0xFFFF) as u32) as f32;
                let y = transmute::<u32, i32>(((lparam.0 >> 16) & 0xFFFF) as u32) as f32;

                let (kind, is_drag) = match msg {
                    WM_MOUSEMOVE => {
                        let kind = self
                            .current_click
                            .map(|click| click.kind)
                            .unwrap_or(MouseClickKind::Single);

                        self.last_click =
                            self.last_click.filter(|click| x == click.x && y == click.y);

                        (kind, true)
                    }
                    _ => {
                        let (is_chained_click, previous_kind) = self
                            .last_click
                            .map(|last_click| {
                                (
                                    Some(last_click.button) == button
                                        && self.time - last_click.time <= self.double_click_time,
                                    last_click.kind,
                                )
                            })
                            .unwrap_or((false, MouseClickKind::Single));

                        let kind = if is_chained_click {
                            match previous_kind {
                                MouseClickKind::Single => MouseClickKind::Double,
                                MouseClickKind::Double => MouseClickKind::Triple,
                                MouseClickKind::Triple => MouseClickKind::Single,
                            }
                        } else {
                            MouseClickKind::Single
                        };

                        let click = button.map(|button| RecordedMouseClick {
                            button,
                            kind,
                            x,
                            y,
                            time: self.time,
                        });

                        self.last_click = click;
                        self.current_click = click;

                        (kind, false)
                    }
                };

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
                        button, x, y, has_shift, has_ctrl, false, kind, is_drag,
                    ));
                }
            }
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                const WHEEL_DELTA: f32 = 120.0;

                let delta =
                    transmute::<u16, i16>(((wparam.0 >> 16) & 0xFFFF) as u16) as f32 / WHEEL_DELTA;

                let is_horizontal = msg == WM_MOUSEHWHEEL;

                let x = transmute::<u32, i32>((lparam.0 & 0xFFFF) as u32);
                let y = transmute::<u32, i32>(((lparam.0 >> 16) & 0xFFFF) as u32);

                self.mouse_scrolls.push(MouseScroll {
                    delta,
                    is_horizontal,
                    is_precise: false,
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
