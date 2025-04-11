use terminal_emulator::TerminalEmulator;
use terminal_pane::TerminalPane;

use crate::{
    config::Config,
    editor_buffers::EditorBuffers,
    geometry::rect::Rect,
    input::action::action_name,
    platform::{gfx::Gfx, pty::Pty, window::Window},
    text::{
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
};

use super::{slot_list::SlotList, widget::Widget, Ui};

mod color_table;
mod escape_sequences;
mod terminal_emulator;
mod terminal_pane;

pub struct TerminalDocs {
    normal: Doc,
    alternate: Doc,
}

impl TerminalDocs {
    pub fn new(line_pool: &mut LinePool) -> Self {
        const TERMINAL_DISPLAY_NAME: Option<&str> = Some("Terminal");

        Self {
            normal: Doc::new(None, line_pool, TERMINAL_DISPLAY_NAME, DocKind::Output),
            alternate: Doc::new(None, line_pool, TERMINAL_DISPLAY_NAME, DocKind::Output),
        }
    }

    pub fn clear(&mut self, line_pool: &mut LinePool) {
        self.normal.clear(line_pool);
        self.alternate.clear(line_pool);
    }
}

type Term = (TerminalDocs, TerminalEmulator);

pub struct Terminal {
    pane: TerminalPane,
    term_list: SlotList<Term>,

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

    pub fn layout(&mut self, bounds: Rect, config: &Config, gfx: &mut Gfx) {
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
        ui: &mut Ui,
        window: &mut Window,
        buffers: &mut EditorBuffers,
        config: &Config,
        gfx: &mut Gfx,
        timestamp @ (time, _): (f32, f32),
    ) {
        let mut global_action_handler = window.get_action_handler();

        while let Some(action) = global_action_handler.next(window) {
            match action {
                action_name!(FocusTerminal) => {
                    if self.widget.is_focused(ui, window) {
                        self.widget.release_focus(ui);
                    } else {
                        self.widget.take_focus(ui);
                    }
                }
                _ => global_action_handler.unprocessed(window, action),
            }
        }

        self.pane.update(
            &mut self.widget,
            ui,
            window,
            &mut self.term_list,
            &mut buffers.lines,
        );

        let focused_tab_index = self.pane.focused_tab_index();

        if let Some((tab, (docs, emulator))) = self
            .pane
            .get_tab_with_data_mut(focused_tab_index, &mut self.term_list)
        {
            emulator.update_input(
                &mut self.widget,
                ui,
                window,
                docs,
                tab,
                buffers,
                config,
                gfx,
                time,
            );
        }

        for tab in &mut self.pane.tabs {
            let term_index = tab.data_index();

            let Some((docs, emulator)) = self.term_list.get_mut(term_index) else {
                continue;
            };

            emulator.update_output(
                &mut self.widget,
                ui,
                window,
                docs,
                tab,
                buffers,
                config,
                gfx,
                timestamp,
            );
        }
    }

    pub fn draw(&mut self, ui: &mut Ui, window: &mut Window, gfx: &mut Gfx, config: &Config) {
        let is_focused = self.widget.is_focused(ui, window);

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
