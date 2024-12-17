use terminal_emulator::TerminalEmulator;
use terminal_pane::TerminalPane;

use crate::{
    config::Config,
    geometry::rect::Rect,
    input::{
        key::Key,
        keybind::{Keybind, MOD_CTRL},
    },
    platform::{gfx::Gfx, pty::Pty},
    temp_buffer::TempBuffer,
    text::{cursor::Cursor, doc::Doc, line_pool::LinePool},
};

use super::{slot_list::SlotList, widget::Widget, Ui, UiHandle};

mod terminal_emulator;
mod terminal_pane;

pub struct Terminal {
    pane: TerminalPane,
    term_list: SlotList<(Doc, TerminalEmulator)>,

    pub widget: Widget,
}

impl Terminal {
    pub fn new(ui: &mut Ui, line_pool: &mut LinePool) -> Self {
        let mut term_list = SlotList::new();

        let pane = TerminalPane::new(&mut term_list, line_pool);

        Self {
            pane,
            term_list,

            widget: Widget::new(ui, true),
        }
    }

    pub fn layout(&mut self, bounds: Rect, config: &Config, gfx: &Gfx) {
        let bounds = Rect::new(
            0.0,
            0.0,
            bounds.width,
            gfx.tab_height() + gfx.line_height() * config.terminal_height,
        )
        .at_bottom_of(bounds)
        .floor();

        self.pane.layout(bounds, gfx, &mut self.term_list);

        self.widget.layout(&[bounds]);
    }

    pub fn update(
        &mut self,
        ui: &mut UiHandle,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        cursor_buffer: &mut TempBuffer<Cursor>,
        config: &Config,
        time: f32,
        dt: f32,
    ) {
        let mut global_keybind_handler = ui.window.get_keybind_handler();

        while let Some(keybind) = global_keybind_handler.next(ui.window) {
            match keybind {
                Keybind {
                    key: Key::Grave,
                    mods: MOD_CTRL,
                } => {
                    if self.widget.is_focused(ui) {
                        self.widget.release_focus(ui);
                    } else {
                        self.widget.take_focus(ui);
                    }
                }
                _ => global_keybind_handler.unprocessed(ui.window, keybind),
            }
        }

        self.pane
            .update(&self.widget, ui, &mut self.term_list, line_pool);

        let focused_tab_index = self.pane.focused_tab_index();

        if let Some((tab, (doc, emulator))) = self
            .pane
            .get_tab_with_data_mut(focused_tab_index, &mut self.term_list)
        {
            emulator.update_input(
                &self.widget,
                ui,
                doc,
                tab,
                line_pool,
                text_buffer,
                config,
                time,
            );
        }

        for tab in &mut self.pane.tabs {
            let term_index = tab.data_index();

            let Some((doc, emulator)) = self.term_list.get_mut(term_index) else {
                continue;
            };

            emulator.update_output(ui, doc, tab, line_pool, cursor_buffer, time, dt);
        }
    }

    pub fn draw(&mut self, ui: &mut UiHandle, config: &Config) {
        let is_focused = self.widget.is_focused(ui);
        let gfx = ui.gfx();

        self.pane.draw(
            Some(config.theme.terminal.background),
            &mut self.term_list,
            config,
            gfx,
            is_focused,
        );
    }

    pub fn bounds(&self) -> Rect {
        self.widget.bounds()
    }

    pub fn is_animating(&self) -> bool {
        self.pane.is_animating()
    }

    pub fn ptys(&mut self) -> impl Iterator<Item = &mut Pty> {
        self.term_list
            .iter_mut()
            .filter_map(|term| term.as_mut().and_then(|(_, emulator)| emulator.pty()))
    }
}
