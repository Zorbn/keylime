use terminal_emulator::TerminalEmulator;
use terminal_pane::TerminalPane;

use crate::{
    config::Config,
    ctx::Ctx,
    geometry::rect::Rect,
    input::action::action_name,
    platform::process::Process,
    text::doc::{Doc, DocFlags},
};

use super::{core::WidgetId, slot_list::SlotList};

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
            normal: Doc::new(None, Some(TERMINAL_DISPLAY_NAME.into()), DocFlags::TERMINAL),
            alternate: Doc::new(None, Some(TERMINAL_DISPLAY_NAME.into()), DocFlags::TERMINAL),
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
    pub fn new(parent_id: WidgetId, ctx: &mut Ctx) -> Self {
        let mut term_list = SlotList::new();

        let widget_id = ctx.ui.new_widget(parent_id, Default::default());
        let pane = TerminalPane::new(&mut term_list, widget_id, ctx);

        Self {
            pane,
            term_list,

            widget_id,
        }
    }

    pub fn layout(&mut self, bounds: Rect, config: &Config, ctx: &mut Ctx) {
        let gfx = &mut ctx.gfx;

        let bounds = Rect::new(
            0.0,
            0.0,
            bounds.width,
            gfx.tab_height() + gfx.line_height() * config.terminal_height,
        )
        .at_bottom_of(bounds)
        .floor();

        ctx.ui.widget_mut(self.widget_id).bounds = bounds;

        self.pane.layout(bounds, &mut self.term_list, ctx);
    }

    pub fn update(&mut self, ctx: &mut Ctx) {
        let mut global_action_handler = ctx.window.action_handler();

        while let Some(action) = global_action_handler.next(ctx.window) {
            match action {
                action_name!(FocusTerminal) => {
                    let pane_widget_id = self.pane.widget_id();

                    if ctx.ui.is_focused(pane_widget_id) {
                        ctx.ui.unfocus(pane_widget_id);
                    } else {
                        ctx.ui.focus(pane_widget_id);
                    }
                }
                _ => global_action_handler.unprocessed(ctx.window, action),
            }
        }

        self.pane.update(&mut self.term_list, ctx);

        if let Some((tab, (docs, emulator))) =
            self.pane.get_focused_tab_with_data_mut(&mut self.term_list)
        {
            emulator.update_input(self.widget_id, docs, tab, ctx);
        }

        for tab in self.pane.tabs.iter_mut() {
            let term_id = tab.data_id();

            let Some((docs, emulator)) = self.term_list.get_mut(term_id) else {
                continue;
            };

            emulator.update_output(docs, tab, ctx);
        }
    }

    pub fn update_camera(&mut self, ctx: &mut Ctx, dt: f32) {
        self.pane.update_camera(&mut self.term_list, ctx, dt);
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        self.pane.draw(
            Some(ctx.config.theme.terminal.background),
            &mut self.term_list,
            ctx,
        );
    }

    pub fn is_animating(&self) -> bool {
        self.pane.is_animating()
    }

    pub fn ptys(&mut self) -> impl Iterator<Item = &mut Process> {
        self.term_list
            .iter_mut()
            .filter_map(|(_, emulator)| emulator.pty())
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }
}
