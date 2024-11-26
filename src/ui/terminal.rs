use crate::{
    config::Config,
    geometry::rect::Rect,
    input::{key::Key, keybind::Keybind},
    platform::{gfx::Gfx, pty::Pty, window::Window},
    temp_buffer::TempBuffer,
    text::{
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
};

use super::tab::Tab;

pub struct Terminal {
    tab: Tab,
    doc: Doc,
    pty: Option<Pty>,

    bounds: Rect,
}

impl Terminal {
    pub fn new(line_pool: &mut LinePool) -> Self {
        Self {
            tab: Tab::new(0),
            doc: Doc::new(line_pool, DocKind::Output),
            pty: Pty::new(40, 24).ok(),

            bounds: Rect::zero(),
        }
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx) {
        let height = (gfx.line_height() * 15.0).floor();

        self.bounds = Rect::new(bounds.x, bounds.bottom() - height, bounds.width, height);

        self.tab.layout(Rect::zero(), self.bounds, &self.doc, gfx);
    }

    pub fn update(
        &mut self,
        window: &mut Window,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
        dt: f32,
    ) {
        let Some(pty) = self.pty.as_mut() else {
            return;
        };

        let mut char_handler = window.get_char_handler();

        while let Some(c) = char_handler.next(window) {
            pty.input.push(c as u32);
        }

        let mut keybind_handler = window.get_keybind_handler();

        while let Some(keybind) = keybind_handler.next(window) {
            match keybind {
                Keybind {
                    key: Key::Enter, ..
                } => {
                    pty.input.extend_from_slice(&['\r' as u32, '\n' as u32]);
                }
                Keybind {
                    key: Key::Backspace,
                    ..
                } => {
                    pty.input.extend_from_slice(&[0x7F]);
                }
                _ => {}
            }
        }

        pty.flush();

        if let Ok(mut output) = pty.output.try_lock() {
            for c in output.iter() {
                if let Some(c) = char::from_u32(*c) {
                    let start = self.doc.end();
                    self.doc.insert(start, &[c], line_pool, time);
                }
            }

            output.clear();
        }

        self.tab
            .update(&mut self.doc, window, line_pool, text_buffer, config, time);

        self.tab.update_camera(&self.doc, window, dt);
    }

    pub fn draw(&mut self, config: &Config, gfx: &mut Gfx, is_focused: bool) {
        self.tab.draw(&mut self.doc, config, gfx, is_focused);
    }

    pub fn on_close(&mut self) {
        self.pty.take();
    }

    pub fn pty(&self) -> Option<&Pty> {
        self.pty.as_ref()
    }

    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn is_animating(&self) -> bool {
        self.tab.is_animating()
    }
}
