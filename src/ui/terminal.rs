use terminal_emulator::TerminalEmulator;
use terminal_pane::TerminalPane;

use crate::{
    ctx::Ctx,
    geometry::rect::Rect,
    input::action::action_name,
    platform::process::Process,
    text::doc::{Doc, DocFlags},
};

use super::{core::WidgetId, pane_list::PaneList, slot_list::SlotList};

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
    panes: PaneList<TerminalPane, Term>,
    term_list: SlotList<Term>,

    widget_id: WidgetId,
}

impl Terminal {
    pub fn new(parent_id: WidgetId, ctx: &mut Ctx) -> Self {
        let mut terminal = Self {
            panes: PaneList::new(),
            term_list: SlotList::new(),

            widget_id: ctx.ui.new_widget(parent_id, Default::default()),
        };

        terminal.add_pane(ctx);

        terminal
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.panes.is_animating(ctx)
    }

    // pub fn layout(&mut self, bounds: Rect, ctx: &mut Ctx) {
    //     let gfx = &mut ctx.gfx;

    //     let bounds = Rect::new(
    //         0.0,
    //         0.0,
    //         bounds.width,
    //         gfx.tab_height() + gfx.line_height() * ctx.config.terminal_height,
    //     )
    //     .at_bottom_of(bounds)
    //     .floor();

    //     ctx.ui.widget_mut(self.widget_id).bounds = bounds;

    //     self.panes.layout(bounds, &mut self.term_list, ctx);
    // }

    pub fn update(&mut self, ctx: &mut Ctx) {
        self.panes.update(self.widget_id, ctx);

        self.handle_actions(ctx);

        let pane = self.panes.get_last_focused_mut(ctx.ui).unwrap();
        let pane_widget_id = pane.widget_id();

        let mut global_action_handler = ctx.window.action_handler();

        while let Some(action) = global_action_handler.next(ctx) {
            match action {
                action_name!(FocusTerminal) => {
                    if ctx.ui.is_in_focused_hierarchy(self.widget_id) {
                        ctx.ui.unfocus_hierarchy(self.widget_id);
                    } else {
                        ctx.ui.focus(pane_widget_id);
                    }
                }
                _ => global_action_handler.unprocessed(ctx.window, action),
            }
        }

        pane.update(&mut self.term_list, ctx);

        if let Some((tab, (docs, emulator))) =
            pane.get_focused_tab_with_data_mut(&mut self.term_list)
        {
            emulator.update_input(pane_widget_id, docs, tab, ctx);
        }

        for tab in pane.tabs.iter_mut() {
            let term_id = tab.data_id();

            let Some((docs, emulator)) = self.term_list.get_mut(term_id) else {
                continue;
            };

            emulator.update_output(docs, tab, ctx);
        }

        self.panes
            .remove_excess(ctx.ui, |pane| pane.tabs.is_empty());
    }

    fn handle_actions(&mut self, ctx: &mut Ctx) {
        let mut action_handler = ctx.ui.action_handler(self.widget_id, ctx.window);

        while let Some(action) = action_handler.next(ctx) {
            match action {
                action_name!(NewPane) => self.add_pane(ctx),
                action_name!(ClosePane) => self.close_pane(ctx),
                _ => action_handler.unprocessed(ctx.window, action),
            }
        }
    }

    pub fn animate(&mut self, ctx: &mut Ctx, dt: f32) {
        self.panes.animate(&mut self.term_list, ctx, dt);
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        self.panes.draw(
            Some(ctx.config.theme.terminal.background),
            &mut self.term_list,
            ctx,
        );
    }

    fn add_pane(&mut self, ctx: &mut Ctx) {
        let pane = TerminalPane::new(&mut self.term_list, self.widget_id, ctx);

        self.panes.add(pane, ctx.ui);

        let bounds = ctx.ui.bounds(self.widget_id);
        // TODO:
        // self.layout(bounds, ctx);
    }

    fn close_pane(&mut self, ctx: &mut Ctx) {
        if self.panes.len() == 1 {
            return;
        }

        self.panes.remove(ctx.ui);
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
