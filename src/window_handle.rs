use std::ptr::copy_nonoverlapping;

use windows::{
    core::Result,
    Win32::{
        Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND},
        System::{
            DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard, SetClipboardData},
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
            Ole::CF_UNICODETEXT,
            Performance::QueryPerformanceCounter,
        },
        UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
        },
    },
};

use crate::{
    defer,
    gfx::Gfx,
    input_handlers::{CharHandler, KeybindHandler, MouseScrollHandler, MousebindHandler},
    keybind::Keybind,
    mouse_scroll::MouseScroll,
    mousebind::Mousebind,
};

pub struct WindowHandle<'a> {
    timer_frequency: &'a mut i64,
    last_queried_time: &'a mut Option<i64>,
    time: &'a mut f32,

    hwnd: &'a HWND,

    is_running: &'a mut bool,
    is_focused: &'a mut bool,

    dpi: &'a mut f32,
    width: &'a mut i32,
    height: &'a mut i32,

    wide_text_buffer: &'a mut Vec<u16>,
    text_buffer: &'a mut Vec<char>,

    gfx: &'a mut Option<Gfx>,

    pub chars_typed: &'a mut Vec<char>,
    pub keybinds_typed: &'a mut Vec<Keybind>,
    pub mousebinds_pressed: &'a mut Vec<Mousebind>,
    pub mouse_scrolls: &'a mut Vec<MouseScroll>,

    was_copy_implicit: &'a mut bool,
    did_just_copy: &'a mut bool,
}

impl<'a> WindowHandle<'a> {
    pub fn new(
        timer_frequency: &'a mut i64,
        last_queried_time: &'a mut Option<i64>,
        time: &'a mut f32,

        hwnd: &'a HWND,

        is_running: &'a mut bool,
        is_focused: &'a mut bool,

        dpi: &'a mut f32,
        width: &'a mut i32,
        height: &'a mut i32,

        wide_text_buffer: &'a mut Vec<u16>,
        text_buffer: &'a mut Vec<char>,

        gfx: &'a mut Option<Gfx>,

        chars_typed: &'a mut Vec<char>,
        keybinds_typed: &'a mut Vec<Keybind>,
        mousebinds_pressed: &'a mut Vec<Mousebind>,
        mouse_scrolls: &'a mut Vec<MouseScroll>,

        was_copy_implicit: &'a mut bool,
        did_just_copy: &'a mut bool,
    ) -> Self {
        Self {
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
        }
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
                let _ = GetMessageW(&mut msg, *self.hwnd, 0, 0);
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            };

            while PeekMessageW(&mut msg, *self.hwnd, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let mut queried_time = 0i64;
            let _ = QueryPerformanceCounter(&mut queried_time);

            let dt = if let Some(last_queried_time) = *self.last_queried_time {
                (queried_time - last_queried_time) as f32 / *self.timer_frequency as f32
            } else {
                0.0
            };

            *self.last_queried_time = Some(queried_time);

            *self.time += dt;

            // Don't return massive delta times from big gaps in animation, because those
            // might cause visual jumps or other problems. (eg. if you don't interact with
            // the app for 15 seconds and then you do something that starts an animation,
            // that animation shouldn't instantly jump to completion).
            (*self.time, if is_animating { dt } else { 0.0 })
        }
    }

    pub fn is_running(&self) -> bool {
        *self.is_running
    }

    pub fn is_focused(&self) -> bool {
        *self.is_focused
    }

    pub fn hwnd(&self) -> HWND {
        *self.hwnd
    }

    pub fn dpi(&self) -> f32 {
        *self.dpi
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
        *self.was_copy_implicit = was_copy_implicit;
        *self.did_just_copy = true;

        self.wide_text_buffer.clear();

        for c in text {
            let mut dst = [0u16; 2];

            for wide_c in c.encode_utf16(&mut dst) {
                self.wide_text_buffer.push(*wide_c);
            }
        }

        self.wide_text_buffer.push(0);

        unsafe {
            OpenClipboard(*self.hwnd)?;

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
            OpenClipboard(*self.hwnd)?;

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
        *self.was_copy_implicit
    }
}
