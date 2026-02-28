use terminal_emulator::TerminalEmulator;
use terminal_pane::TerminalPane;

use crate::{
    ctx::Ctx,
    input::action::action_name,
    platform::process::Process,
    text::doc::{Doc, DocFlags},
    ui::{
        core::{WidgetScale, WidgetSettings},
        msg::Msg,
    },
};

use super::{core::WidgetId, pane_list::PaneList, slot_list::SlotList};

mod color_table;
mod escape_parser;
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
    panes: PaneList<TerminalPane, Term>,
    term_list: SlotList<Term>,

    widget_id: WidgetId,
}

impl Terminal {
    pub fn new(parent_id: WidgetId, ctx: &mut Ctx) -> Self {
        let widget_id = ctx.ui.new_widget(
            parent_id,
            WidgetSettings {
                scale: WidgetScale::Fractional(0.5),
                ..Default::default()
            },
        );

        let mut terminal = Self {
            panes: PaneList::new(widget_id, ctx.ui),
            term_list: SlotList::new(),

            widget_id,
        };

        terminal.add_pane(ctx);

        terminal
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.panes.is_animating(ctx)
    }

    pub fn receive_msgs(&mut self, ctx: &mut Ctx) {
        while let Some(msg) = ctx.ui.msg(self.widget_id) {
            match msg {
                Msg::Action(action_name!(NewPane)) => self.add_pane(ctx),
                Msg::Action(action_name!(ClosePane)) => self.close_pane(ctx),
                _ => ctx.ui.skip(self.widget_id, msg),
            }
        }

        self.panes.receive_msgs(&mut self.term_list, ctx);
    }

    pub fn update(&mut self, ctx: &mut Ctx, dt: f32) {
        self.panes.update(&mut self.term_list, ctx, dt);
        self.panes.remove_excess(ctx.ui, |pane| !pane.has_tabs());
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        self.panes.draw(
            Some(ctx.config.theme.terminal.background),
            &mut self.term_list,
            ctx,
        );
    }

    fn add_pane(&mut self, ctx: &mut Ctx) {
        let pane = TerminalPane::new(&mut self.term_list, self.panes.widget_id(), ctx);
        self.panes.add(pane, ctx.ui);
    }

    fn close_pane(&mut self, ctx: &mut Ctx) {
        if self.panes.len() == 1 {
            return;
        }

        self.panes.remove_focused(ctx.ui);
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
