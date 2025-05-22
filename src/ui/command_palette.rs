mod all_actions_mode;
mod all_diagnostics_mode;
mod all_files_mode;
mod file_explorer_mode;
pub mod find_in_files_mode;
mod go_to_line_mode;
mod incremental_results;
mod mode;
pub mod references_mode;
pub mod rename_mode;
mod search_mode;

use std::path::PathBuf;

use crate::{
    ctx::Ctx,
    geometry::{
        position::Position,
        rect::Rect,
        sides::{Side, Sides},
    },
    input::action::{action_name, ActionName},
    lsp::{position_encoding::PositionEncoding, types::EncodedPosition},
    pool::Pooled,
    text::doc::{Doc, DocFlags},
};

use super::{
    core::{Ui, WidgetId, WidgetSettings},
    editor::Editor,
    result_list::{ResultList, ResultListInput, ResultListSubmitKind},
    slot_list::SlotId,
    tab::Tab,
};

use all_actions_mode::AllActionsMode;
use all_diagnostics_mode::AllDiagnosticsMode;
use all_files_mode::AllFilesMode;
use file_explorer_mode::FileExplorerMode;
use find_in_files_mode::FindInFilesMode;
use go_to_line_mode::GoToLineMode;
use mode::{CommandPaletteEventArgs, CommandPaletteMode};
use search_mode::{SearchAndReplaceMode, SearchMode};

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
    PathWithEncodedPosition {
        path: Pooled<PathBuf>,
        encoding: PositionEncoding,
        position: EncodedPosition,
    },
}

pub enum CommandPaletteAction {
    Stay,
    Close,
}

const MAX_VISIBLE_RESULTS: usize = 20;

pub struct CommandPalette {
    mode: Option<Box<dyn CommandPaletteMode>>,
    tab: Tab,
    doc: Doc,
    last_updated_version: Option<usize>,

    result_list: ResultList<CommandPaletteResult>,

    title_bounds: Rect,
    input_bounds: Rect,

    widget_id: WidgetId,
}

impl CommandPalette {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        let widget_id = ui.new_widget(
            parent_id,
            WidgetSettings {
                is_shown: true,
                ..Default::default()
            },
        );

        Self {
            mode: None,
            tab: Tab::new(SlotId::ZERO),
            doc: Doc::new(None, None, DocFlags::SINGLE_LINE),
            last_updated_version: None,

            result_list: ResultList::new(MAX_VISIBLE_RESULTS, true, widget_id, ui),

            title_bounds: Rect::ZERO,
            input_bounds: Rect::ZERO,

            widget_id,
        }
    }

    pub fn is_animating(&self) -> bool {
        self.result_list.is_animating()
            || self.tab.is_animating()
            || self.mode.as_ref().is_some_and(|mode| mode.is_animating())
    }

    pub fn layout(&mut self, bounds: Rect, ctx: &mut Ctx) {
        let Some(mode) = &self.mode else {
            return;
        };

        let gfx = &mut ctx.gfx;

        let title = mode.title();
        let title_padding_x = gfx.glyph_width();
        let title_width =
            gfx.measure_text(title) as f32 * gfx.glyph_width() + title_padding_x * 2.0;

        self.title_bounds = Rect::new(0.0, 0.0, title_width, gfx.tab_height()).floor();

        self.input_bounds = Rect::new(0.0, 0.0, gfx.glyph_width() * 64.0, gfx.line_height() * 2.0)
            .below(self.title_bounds)
            .shift_y(-gfx.border_width())
            .floor();

        self.result_list.layout(
            Rect::new(0.0, 0.0, self.input_bounds.width, 0.0)
                .below(self.input_bounds)
                .shift_y(-gfx.border_width()),
            ctx.ui,
            gfx,
        );

        let result_list_bounds = ctx.ui.widget(self.result_list.widget_id()).bounds;

        ctx.ui.widget_mut(self.widget_id).bounds = self
            .title_bounds
            .expand_to_include(self.input_bounds)
            .expand_to_include(result_list_bounds)
            .center_x_in(bounds)
            .offset_by(Rect::new(0.0, gfx.tab_height() * 2.0, 0.0, 0.0))
            .floor();

        let bounds = ctx.ui.widget(self.widget_id).bounds;

        self.result_list.offset_by(bounds, ctx.ui);

        self.tab.layout(
            Rect::ZERO,
            Rect::new(0.0, 0.0, gfx.glyph_width() * 10.0, gfx.line_height())
                .center_in(self.input_bounds)
                .expand_width_in(self.input_bounds)
                .offset_by(bounds)
                .floor(),
            0.0,
            &self.doc,
            gfx,
        );
    }

    pub fn update(&mut self, editor: &mut Editor, ctx: &mut Ctx) {
        if ctx.ui.is_visible(self.widget_id) && !ctx.ui.is_in_focused_hierarchy(self.widget_id) {
            self.close(ctx.ui);
        }

        let mut global_action_handler = ctx.window.action_handler();

        while let Some(action) = global_action_handler.next(ctx) {
            match action {
                action_name!(OpenAllActions) => {
                    self.open(Box::new(AllActionsMode), editor, ctx);
                }
                action_name!(OpenFileExplorer) => {
                    self.open(Box::new(FileExplorerMode::new()), editor, ctx);
                }
                action_name!(OpenSearch) => {
                    self.open(Box::new(SearchMode::new()), editor, ctx);
                }
                action_name!(OpenSearchAndReplace) => {
                    self.open(Box::new(SearchAndReplaceMode::new()), editor, ctx);
                }
                action_name!(OpenFindInFiles) => {
                    self.open(Box::new(FindInFilesMode::new()), editor, ctx);
                }
                action_name!(OpenAllFiles) => {
                    self.open(Box::new(AllFilesMode::new()), editor, ctx);
                }
                action_name!(OpenAllDiagnostics) => {
                    self.open(Box::new(AllDiagnosticsMode), editor, ctx);
                }
                action_name!(OpenGoToLine) => {
                    self.open(Box::new(GoToLineMode), editor, ctx);
                }
                _ => global_action_handler.unprocessed(ctx.window, action),
            }
        }

        if let Some(mut mode) = self.mode.take() {
            let mut action_handler = ctx.ui.action_handler(self.widget_id, ctx.window);

            while let Some(action) = action_handler.next(ctx) {
                if !mode.on_action(self, CommandPaletteEventArgs::new(editor, ctx), action) {
                    action_handler.unprocessed(ctx.window, action);
                }
            }

            mode.on_update(self, CommandPaletteEventArgs::new(editor, ctx));
            self.mode = Some(mode);
        }

        let result_input = self.result_list.update(ctx);

        match result_input {
            ResultListInput::None => {}
            ResultListInput::Complete => self.complete_result(editor, ctx),
            ResultListInput::Submit { kind } => {
                self.submit(kind, editor, ctx);
            }
            ResultListInput::Close => self.close(ctx.ui),
        }

        self.tab.update(self.widget_id, &mut self.doc, ctx);
        self.update_results(editor, ctx);
    }

    pub fn update_camera(&mut self, ctx: &mut Ctx, dt: f32) {
        self.tab.update_camera(self.widget_id, &self.doc, ctx, dt);
        self.result_list.update_camera(ctx.ui, dt);
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

        let is_focused = ctx.ui.is_focused(self.widget_id);
        let widget = ctx.ui.widget(self.widget_id);

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        gfx.begin(Some(widget.bounds));

        gfx.add_bordered_rect(
            self.input_bounds,
            Sides::ALL,
            theme.background,
            theme.border,
        );

        gfx.add_bordered_rect(
            self.title_bounds,
            Sides::ALL.without(Side::Bottom),
            theme.background,
            theme.border,
        );

        gfx.add_rect(
            self.title_bounds.top_border(gfx.border_width()),
            theme.keyword,
        );

        gfx.add_text(
            mode.title(),
            gfx.glyph_width(),
            gfx.border_width() + gfx.tab_padding_y(),
            theme.normal,
        );

        let doc_bounds = self.tab.doc_bounds();

        gfx.add_bordered_rect(
            doc_bounds
                .add_margin(gfx.border_width())
                .unoffset_by(widget.bounds),
            Sides::ALL,
            theme.background,
            theme.border,
        );

        gfx.end();

        self.tab
            .draw(Default::default(), &mut self.doc, is_focused, ctx);

        self.result_list
            .draw(ctx, |result, theme| mode.on_display_result(result, theme));
    }

    pub fn open(
        &mut self,
        mut mode: Box<dyn CommandPaletteMode>,
        editor: &mut Editor,
        ctx: &mut Ctx,
    ) {
        self.doc.clear(ctx);
        self.result_list.reset();
        self.last_updated_version = None;
        self.mode = None;

        ctx.ui.focus(self.widget_id);

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
}
