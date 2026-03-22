pub mod all_actions_mode;
pub mod all_diagnostics_mode;
pub mod all_files_mode;
pub mod file_explorer_mode;
pub mod find_in_files_mode;
pub mod go_to_line_mode;
mod incremental_results;
mod mode;
pub mod references_mode;
pub mod rename_mode;
pub mod search_mode;

use std::path::PathBuf;

use crate::{
    ctx::Ctx,
    geometry::{
        position::Position,
        rect::Rect,
        sides::{Side, Sides},
    },
    input::{action::ActionName, editing_actions::handle_select_all},
    lsp::{position_encoding::PositionEncoding, types::EncodedPosition},
    platform::gfx::Gfx,
    pool::Pooled,
    text::doc::{Doc, DocFlags},
    ui::msg::Msg,
};

use super::{
    core::{Ui, WidgetId, WidgetSettings},
    editor::Editor,
    result_list::{ResultList, ResultListInput, ResultListSubmitKind},
    slot_list::SlotId,
    tab::Tab,
};

use mode::{CommandPaletteEventArgs, CommandPaletteMode};

pub struct CommandPaletteResult {
    pub text: Pooled<String>,
    pub meta_data: CommandPaletteMetaData,
}

pub enum CommandPaletteMetaData {
    ActionName(ActionName),
    Path(Pooled<PathBuf>),
    PathWithPosition {
        path: Pooled<PathBuf>,
        position: Position,
    },
    DiagnosticWithPosition {
        path: Pooled<PathBuf>,
        position: Position,
        severity: usize,
    },
    DiagnosticWithEncodedPosition {
        path: Pooled<PathBuf>,
        encoding: PositionEncoding,
        position: EncodedPosition,
        severity: usize,
    },
}

pub enum CommandPaletteAction {
    Stay,
    Close,
}

pub struct CommandPalette {
    mode: Option<Box<dyn CommandPaletteMode>>,
    tab: Tab,
    doc: Doc,
    last_updated_version: Option<usize>,

    result_list: ResultList<CommandPaletteResult>,

    parent_bounds: Rect,

    widget_id: WidgetId,
}

impl CommandPalette {
    const MAX_VISIBLE_RESULTS: usize = 20;

    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        let widget_id = ui.new_widget(
            parent_id,
            WidgetSettings {
                is_shown: false,
                popup: Some(Rect::ZERO),
                ..Default::default()
            },
        );

        let tab = Tab::new(widget_id, SlotId::ZERO, ui);
        let result_list = ResultList::new(tab.widget_id(), ui);

        Self {
            mode: None,
            tab,
            doc: Doc::new(None, None, DocFlags::SINGLE_LINE),
            last_updated_version: None,

            result_list,
            parent_bounds: Rect::ZERO,

            widget_id,
        }
    }

    pub fn is_animating(&self, ctx: &Ctx) -> bool {
        self.result_list.is_animating()
            || self.tab.is_animating(ctx)
            || self.mode.as_ref().is_some_and(|mode| mode.is_animating())
    }

    pub fn receive_msgs(&mut self, editor: &mut Editor, ctx: &mut Ctx) {
        while let Some(msg) = ctx.ui.msg(self.widget_id) {
            match msg {
                Msg::PopupParentResized { bounds } => self.parent_bounds = bounds,
                Msg::GainedFocus => ctx.ui.focus(self.result_list.widget_id()),
                Msg::Action(action) => {
                    let Some(mut mode) = self.mode.take() else {
                        continue;
                    };

                    if !mode.on_action(self, CommandPaletteEventArgs::new(editor, ctx), action) {
                        ctx.ui.skip(self.widget_id, msg);
                    }

                    self.mode = Some(mode);
                }
                _ => ctx.ui.skip(self.widget_id, msg),
            }
        }

        let result_input = self.result_list.receive_msgs(ctx);

        match result_input {
            ResultListInput::Complete => self.complete_result(editor, ctx),
            ResultListInput::Submit { kind } => self.submit(kind, editor, ctx),
            ResultListInput::Close => self.close(ctx.ui),
            _ => {}
        }

        while let Some(msg) = ctx.ui.msg(self.tab.widget_id()) {
            match msg {
                Msg::GainedFocus => ctx.ui.focus(self.result_list.widget_id()),
                _ => self.tab.receive_msg(msg, &mut self.doc, ctx),
            }
        }
    }

    fn update_popups(&mut self, ctx: &mut Ctx) {
        let title_height = Self::title_height(ctx.gfx);
        let input_height = ctx.gfx.line_height() * 2.0;
        let results_height = self
            .result_list
            .desired_height(Self::MAX_VISIBLE_RESULTS, ctx.gfx);

        let bounds = Rect::new(
            0.0,
            ctx.gfx.tab_height() * 2.0,
            Self::width(ctx.gfx),
            title_height + input_height + results_height - ctx.gfx.border_width(),
        )
        .center_x_in(self.parent_bounds);

        ctx.ui.set_popup(self.widget_id, Some(bounds));

        ctx.ui.set_popup(
            self.tab.widget_id(),
            Some(Rect::new(
                bounds.x + ctx.gfx.glyph_width(),
                bounds.y + title_height - ctx.gfx.border_width() + ctx.gfx.line_height() / 2.0,
                bounds.width - ctx.gfx.glyph_width() * 2.0,
                ctx.gfx.line_height(),
            )),
        );

        ctx.ui.set_popup(
            self.result_list.widget_id(),
            Some(Rect::new(
                bounds.x,
                bounds.y + title_height + input_height - ctx.gfx.border_width() * 2.0,
                bounds.width,
                results_height,
            )),
        )
    }

    pub fn update(&mut self, editor: &mut Editor, ctx: &mut Ctx, dt: f32) {
        if ctx.ui.is_visible(self.widget_id) && !ctx.ui.is_focused(self.widget_id) {
            self.close(ctx.ui);
        }

        if let Some(mut mode) = self.mode.take() {
            mode.on_update(self, CommandPaletteEventArgs::new(editor, ctx));
            self.mode = Some(mode);
        }

        self.tab.update(&mut self.doc, ctx, dt);
        self.result_list.update(ctx, dt, |result| &result.text);
        self.update_results(editor, ctx);
        self.update_popups(ctx);
    }

    fn submit(&mut self, kind: ResultListSubmitKind, editor: &mut Editor, ctx: &mut Ctx) {
        self.complete_result(editor, ctx);

        let Some(mut mode) = self.mode.take() else {
            return;
        };

        let action = mode.on_submit(self, CommandPaletteEventArgs::new(editor, ctx), kind);
        self.mode = Some(mode);

        match action {
            CommandPaletteAction::Stay => {}
            CommandPaletteAction::Close => self.close(ctx.ui),
        }
    }

    fn complete_result(&mut self, editor: &mut Editor, ctx: &mut Ctx) {
        let Some(mut mode) = self.mode.take() else {
            return;
        };

        mode.on_complete_result(self, CommandPaletteEventArgs::new(editor, ctx));
        self.mode = Some(mode);

        self.update_results(editor, ctx);
    }

    fn update_results(&mut self, editor: &mut Editor, ctx: &mut Ctx) {
        if Some(self.doc.version()) == self.last_updated_version {
            return;
        }

        self.last_updated_version = Some(self.doc.version());

        let Some(mut mode) = self.mode.take() else {
            return;
        };

        mode.on_update_results(self, CommandPaletteEventArgs::new(editor, ctx));
        self.mode = Some(mode);
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        if !ctx.ui.is_visible(self.widget_id) {
            return;
        }

        let Some(mode) = &self.mode else {
            return;
        };

        self.result_list
            .draw(ctx, |result, theme| mode.on_display_result(result, theme));

        let bounds = ctx.ui.bounds(self.widget_id);

        let ui = &ctx.ui;
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        let title = mode.title();
        let title_padding_x = gfx.glyph_width();
        let title_width =
            gfx.measure_text(title) as f32 * gfx.glyph_width() + title_padding_x * 2.0;
        let title_height = Self::title_height(gfx);

        gfx.begin(Some(bounds));

        let input_bounds = Rect::new(
            0.0,
            title_height - gfx.border_width(),
            bounds.width,
            gfx.line_height() * 2.0,
        );

        gfx.add_bordered_rect(input_bounds, Sides::ALL, theme.background, theme.border);

        let title_bounds = Rect::new(0.0, 0.0, title_width, title_height);

        gfx.add_bordered_rect(
            title_bounds,
            Sides::ALL.without(Side::Bottom),
            theme.background,
            theme.border,
        );

        gfx.add_rect(title_bounds.top_border(gfx.border_width()), theme.keyword);

        gfx.add_text(
            mode.title(),
            gfx.glyph_width(),
            gfx.border_width() + gfx.tab_padding_y(),
            theme.normal,
        );

        let doc_bounds = self.tab.doc_bounds(ui);

        gfx.add_bordered_rect(
            doc_bounds
                .add_margin(gfx.border_width())
                .relative_to(bounds),
            Sides::ALL,
            theme.background,
            theme.border,
        );

        gfx.end();

        self.tab.draw(Default::default(), &mut self.doc, ctx);
    }

    pub fn open(
        &mut self,
        mut mode: Box<dyn CommandPaletteMode>,
        editor: &mut Editor,
        ctx: &mut Ctx,
    ) {
        ctx.ui.focus(self.widget_id);

        let do_reuse = self.mode.as_ref().is_some_and(|previous_mode| {
            previous_mode.is_reusable() && previous_mode.title() == mode.title()
        });

        if do_reuse {
            handle_select_all(&mut self.doc, ctx.gfx);
            self.update_results(editor, ctx);

            return;
        }

        self.doc.clear(ctx);
        self.tab.skip_cursor_animations(&self.doc, ctx);
        self.result_list.reset();
        self.last_updated_version = None;
        self.mode = None;

        mode.on_open(self, CommandPaletteEventArgs::new(editor, ctx));
        self.mode = Some(mode);

        self.update_results(editor, ctx);
    }

    fn close(&self, ui: &mut Ui) {
        ui.hide(self.widget_id);
    }

    pub fn input(&self) -> &str {
        self.doc.get_line(0).unwrap_or_default()
    }

    fn title_height(gfx: &Gfx) -> f32 {
        gfx.tab_height()
    }

    fn width(gfx: &Gfx) -> f32 {
        gfx.glyph_width() * 64.0
    }
}
