use terminal_emulator::TerminalEmulator;
use terminal_pane::TerminalPane;

use crate::{
    config::Config,
    ctx::Ctx,
    geometry::rect::Rect,
    input::action::action_name,
    platform::process::Process,
    text::doc::{Doc, DocKind},
};

use super::{
    core::{ContainerDirection, WidgetId, WidgetLayout},
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
}

impl Terminal {
    pub fn new() -> Self {
        let mut term_list = SlotList::new();

        let pane = TerminalPane::new(&mut term_list);

        Self { pane, term_list }
    }

    // pub fn layout(&mut self, bounds: Rect, config: &Config, ctx: &mut Ctx) {
    //     let gfx = &mut ctx.gfx;

    //     let bounds = Rect::new(
    //         0.0,
    //         0.0,
    //         bounds.width,
    //         gfx.tab_height() + gfx.line_height() * config.terminal_height,
    //     )
    //     .at_bottom_of(bounds)
    //     .floor();

    //     ctx.ui.widget_mut(self.widget_id).bounds = bounds;

    //     self.pane.layout(bounds, &mut self.term_list, ctx);
    // }

    pub fn update(&mut self, ctx: &mut Ctx, dt: f32) {
        ctx.ui.begin_container(
            WidgetId::Name("Terminal"),
            WidgetLayout {
                height: Some(
                    ctx.gfx.tab_height() + ctx.gfx.line_height() * ctx.config.terminal_height,
                ),
                ..Default::default()
            },
            ContainerDirection::Horizontal,
        );

        let mut global_action_handler = ctx.window.action_handler();

        while let Some(action) = global_action_handler.next(ctx.window) {
            match action {
                action_name!(FocusTerminal) => {
                    // let pane_widget_id = self.pane.widget_id();

                    // if ctx.ui.is_focused(pane_widget_id) {
                    //     ctx.ui.unfocus(pane_widget_id);
                    // } else {
                    //     ctx.ui.focus(pane_widget_id);
                    // }
                }
                _ => global_action_handler.unprocessed(ctx.window, action),
            }
        }

        self.pane.update(&mut self.term_list, ctx, dt);

        ctx.ui.end_container();
    }

    pub fn update_camera(&mut self, ctx: &mut Ctx, dt: f32) {
        self.pane.update_camera(&mut self.term_list, ctx, dt);
    }

    // pub fn draw(&mut self, ctx: &mut Ctx) {
    //     self.pane.draw(
    //         Some(ctx.config.theme.terminal.background),
    //         &mut self.term_list,
    //         ctx,
    //     );
    // }

    pub fn is_animating(&self) -> bool {
        self.pane.is_animating()
    }

    pub fn ptys(&mut self) -> impl Iterator<Item = &mut Process> {
        self.term_list
            .iter_mut()
            .filter_map(|term| term.as_mut().and_then(|(_, emulator)| emulator.pty()))
    }
}
