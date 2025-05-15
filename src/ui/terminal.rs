use terminal_emulator::TerminalEmulator;
use terminal_pane::TerminalPane;

use crate::{
    config::Config,
    ctx::Ctx,
    geometry::rect::Rect,
    input::action::action_name,
    platform::{gfx::Gfx, process::Process},
    text::doc::{Doc, DocKind},
};

use super::{
    core::{Ui, WidgetId},
    slot_list::SlotList,
};

mod color_table;
mod escape_sequences;
mod terminal_emulator;
mod terminal_pane;

pub const TERMINAL_DISPLAY_NAME: &str = "Terminal";

pub struct TerminalDocs {
    normal: Doc,
    alternate: Doc,
}

impl TerminalDocs {
    pub fn new() -> Self {
        Self {
            normal: Doc::new(None, Some(TERMINAL_DISPLAY_NAME.into()), DocKind::Output),
            alternate: Doc::new(None, Some(TERMINAL_DISPLAY_NAME.into()), DocKind::Output),
        }
    }

    pub fn clear(&mut self, ctx: &mut Ctx) {
        self.normal.clear(ctx);
        self.alternate.clear(ctx);
    }
}

type Term = (TerminalDocs, TerminalEmulator);

pub struct Terminal {
    pane: TerminalPane,
    term_list: SlotList<Term>,

    widget_id: WidgetId,
}

impl Terminal {
    pub fn new(ui: &mut Ui) -> Self {
        let mut term_list = SlotList::new();

        let pane = TerminalPane::new(&mut term_list);

        Self {
            pane,
            term_list,

            widget_id: ui.new_widget(true),
        }
    }

    pub fn layout(&mut self, bounds: Rect, config: &Config, ui: &mut Ui, gfx: &mut Gfx) {
        let bounds = Rect::new(
            0.0,
            0.0,
            bounds.width,
            gfx.tab_height() + gfx.line_height() * config.terminal_height,
        )
        .at_bottom_of(bounds)
        .floor();

        self.pane.layout(bounds, gfx, &mut self.term_list);
        ui.widget_mut(self.widget_id).layout(&[bounds]);
    }

    pub fn update(&mut self, ui: &mut Ui, ctx: &mut Ctx) {
        let mut global_action_handler = ctx.window.action_handler();

        while let Some(action) = global_action_handler.next(ctx.window) {
            match action {
                action_name!(FocusTerminal) => {
                    if ui.is_focused(self.widget_id) {
                        ui.unfocus(self.widget_id);
                    } else {
                        ui.focus(self.widget_id);
                    }
                }
                _ => global_action_handler.unprocessed(ctx.window, action),
            }
        }

        self.pane
            .update(self.widget_id, ui, &mut self.term_list, ctx);

        let focused_tab_index = self.pane.focused_tab_index();

        if let Some((tab, (docs, emulator))) = self
            .pane
            .get_tab_with_data_mut(focused_tab_index, &mut self.term_list)
        {
            emulator.update_input(self.widget_id, ui, docs, tab, ctx);
        }

        for tab in self.pane.tabs.iter_mut() {
            let term_index = tab.data_index();

            let Some((docs, emulator)) = self.term_list.get_mut(term_index) else {
                continue;
            };

            emulator.update_output(docs, tab, ctx);
        }
    }

    pub fn update_camera(&mut self, ui: &mut Ui, ctx: &mut Ctx, dt: f32) {
        self.pane
            .update_camera(self.widget_id, ui, &mut self.term_list, ctx, dt);
    }

    pub fn draw(&mut self, ui: &Ui, ctx: &mut Ctx) {
        let is_focused = ui.is_focused(self.widget_id);

        self.pane.draw(
            Some(ctx.config.theme.terminal.background),
            &mut self.term_list,
            ctx,
            is_focused,
        );
    }

    pub fn is_animating(&self) -> bool {
        self.pane.is_animating()
    }

    pub fn ptys(&mut self) -> impl Iterator<Item = &mut Process> {
        self.term_list
            .iter_mut()
            .filter_map(|term| term.as_mut().and_then(|(_, emulator)| emulator.pty()))
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }
}
