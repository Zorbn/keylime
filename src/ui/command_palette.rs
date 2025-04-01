pub mod file_mode;
pub mod go_to_line_mode;
mod mode;
pub mod search_mode;

use crate::{
    config::Config,
    editor_buffers::EditorBuffers,
    geometry::{
        rect::Rect,
        side::{SIDE_ALL, SIDE_LEFT, SIDE_RIGHT, SIDE_TOP},
    },
    input::action::{action_keybind, action_name},
    platform::gfx::Gfx,
    text::{
        cursor_index::CursorIndex,
        doc::{Doc, DocKind},
        line_pool::LinePool,
    },
};

use super::{
    editor::Editor,
    result_list::{ResultList, ResultListInput, ResultListSubmitKind},
    tab::Tab,
    widget::{Widget, WidgetHandle},
    Ui, UiHandle,
};

use file_mode::MODE_OPEN_FILE;
use go_to_line_mode::MODE_GO_TO_LINE;
use mode::{CommandPaletteEventArgs, CommandPaletteMode};
use search_mode::{MODE_SEARCH, MODE_SEARCH_AND_REPLACE_START};

macro_rules! temp_args {
    ($args:ident) => {
        CommandPaletteEventArgs {
            pane: $args.pane,
            doc_list: $args.doc_list,
            config: $args.config,
            line_pool: $args.line_pool,
            time: $args.time,
        }
    };
}

#[derive(Clone, Copy)]
pub enum CommandPaletteAction {
    Stay,
    Close,
    Open(&'static CommandPaletteMode),
}

const MAX_VISIBLE_RESULTS: usize = 20;

pub struct CommandPalette {
    mode: &'static CommandPaletteMode,
    tab: Tab,
    doc: Doc,
    last_updated_version: Option<usize>,

    result_list: ResultList<String>,
    previous_results: Vec<String>,

    title_bounds: Rect,
    input_bounds: Rect,

    pub widget: Widget,
}

impl CommandPalette {
    pub fn new(ui: &mut Ui, line_pool: &mut LinePool) -> Self {
        Self {
            mode: MODE_OPEN_FILE,
            tab: Tab::new(0),
            doc: Doc::new(line_pool, None, DocKind::SingleLine),
            last_updated_version: None,

            result_list: ResultList::new(MAX_VISIBLE_RESULTS),
            previous_results: Vec::new(),

            title_bounds: Rect::zero(),
            input_bounds: Rect::zero(),

            widget: Widget::new(ui, false),
        }
    }

    pub fn is_animating(&self) -> bool {
        self.result_list.is_animating() || self.tab.is_animating()
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx) {
        let title = self.mode.title;
        let title_padding_x = gfx.glyph_width();
        let title_width =
            Gfx::measure_text(title) as f32 * gfx.glyph_width() + title_padding_x * 2.0;

        self.title_bounds = Rect::new(0.0, 0.0, title_width, gfx.tab_height()).floor();

        self.input_bounds = Rect::new(0.0, 0.0, gfx.glyph_width() * 64.0, gfx.line_height() * 2.0)
            .below(self.title_bounds)
            .shift_y(-gfx.border_width())
            .floor();

        self.result_list.layout(
            Rect::new(0.0, 0.0, self.input_bounds.width, 0.0)
                .below(self.input_bounds)
                .shift_y(-gfx.border_width()),
            gfx,
        );

        self.widget.layout(&[self
            .title_bounds
            .expand_to_include(self.input_bounds)
            .expand_to_include(self.result_list.bounds())
            .center_x_in(bounds)
            .offset_by(Rect::new(0.0, gfx.tab_height() * 2.0, 0.0, 0.0))
            .floor()]);

        self.result_list.offset_by(self.widget.bounds());

        self.tab.layout(
            Rect::zero(),
            Rect::new(0.0, 0.0, gfx.glyph_width() * 10.0, gfx.line_height())
                .center_in(self.input_bounds)
                .expand_width_in(self.input_bounds)
                .offset_by(self.widget.bounds())
                .floor(),
            &self.doc,
            gfx,
        );
    }

    pub fn update(
        &mut self,
        ui: &mut UiHandle,
        editor: &mut Editor,
        buffers: &mut EditorBuffers,
        config: &Config,
        (time, dt): (f32, f32),
    ) {
        if self.widget.is_visible && !self.widget.is_focused(ui) {
            self.close(ui, &mut buffers.lines);
        }

        let args = CommandPaletteEventArgs::new(editor, buffers, config, time);

        let mut global_action_handler = ui.window.get_action_handler();

        while let Some(action) = global_action_handler.next(ui.window) {
            let args = temp_args!(args);

            match action {
                action_name!(OpenCommandPalette) => {
                    self.open(ui, MODE_OPEN_FILE, args);
                }
                action_name!(OpenSearch) => {
                    self.open(ui, MODE_SEARCH, args);
                }
                action_name!(OpenSearchAndReplace) => {
                    self.open(ui, MODE_SEARCH_AND_REPLACE_START, args);
                }
                action_name!(OpenGoToLine) => {
                    self.open(ui, MODE_GO_TO_LINE, args);
                }
                _ => global_action_handler.unprocessed(ui.window, action),
            }
        }

        let mut action_handler = self.widget.get_action_handler(ui);

        while let Some(action) = action_handler.next(ui.window) {
            match action {
                action_keybind!(key: Backspace) => {
                    let on_backspace = self.mode.on_backspace;

                    if !(on_backspace)(self, temp_args!(args)) {
                        action_handler.unprocessed(ui.window, action);
                    }
                }
                _ => action_handler.unprocessed(ui.window, action),
            }
        }

        self.result_list.do_allow_delete = self.doc.cursors_len() == 1
            && self.doc.get_cursor(CursorIndex::Main).position == self.doc.end();

        let mut widget = WidgetHandle::new(&mut self.widget, ui);
        let result_input = self.result_list.update(&mut widget, true, true, dt);

        match result_input {
            ResultListInput::None => {}
            ResultListInput::Complete => self.complete_result(args),
            ResultListInput::Submit { kind } => {
                self.submit(ui, kind, args);
            }
            ResultListInput::Close => self.close(ui, args.line_pool),
        }

        let mut widget = WidgetHandle::new(&mut self.widget, ui);

        self.tab
            .update(&mut widget, &mut self.doc, buffers, config, time);

        self.tab.update_camera(&mut widget, &self.doc, dt);

        let args = CommandPaletteEventArgs::new(editor, buffers, config, time);
        self.update_results(args);
    }

    fn submit(
        &mut self,
        ui: &mut UiHandle,
        kind: ResultListSubmitKind,
        args: CommandPaletteEventArgs,
    ) {
        self.complete_result(temp_args!(args));

        let on_submit = self.mode.on_submit;
        let action = (on_submit)(self, temp_args!(args), kind);

        match action {
            CommandPaletteAction::Stay => {}
            CommandPaletteAction::Close | CommandPaletteAction::Open(_) => {
                if self.mode.do_passthrough_result {
                    for line in self.doc.drain(args.line_pool) {
                        self.previous_results.push(line);
                    }
                } else {
                    self.previous_results.clear();
                }

                self.close(ui, args.line_pool);
            }
        }

        if let CommandPaletteAction::Open(mode) = action {
            self.open(ui, mode, args);
        }
    }

    fn complete_result(&mut self, args: CommandPaletteEventArgs) {
        let on_complete_result = self.mode.on_complete_result;
        (on_complete_result)(self, args);

        self.result_list.drain();
    }

    fn update_results(&mut self, args: CommandPaletteEventArgs) {
        if Some(self.doc.version()) == self.last_updated_version {
            return;
        }

        self.last_updated_version = Some(self.doc.version());

        self.result_list.drain();

        let on_update_results = self.mode.on_update_results;
        (on_update_results)(self, args);
    }

    pub fn draw(&mut self, ui: &mut UiHandle, config: &Config) {
        if !self.widget.is_visible {
            return;
        }

        let is_focused = self.widget.is_focused(ui);
        let gfx = ui.gfx();

        gfx.begin(Some(self.widget.bounds()));

        gfx.add_bordered_rect(
            self.input_bounds,
            SIDE_ALL,
            config.theme.background,
            config.theme.border,
        );

        gfx.add_bordered_rect(
            self.title_bounds,
            SIDE_LEFT | SIDE_RIGHT | SIDE_TOP,
            config.theme.background,
            config.theme.border,
        );

        gfx.add_rect(
            self.title_bounds.top_border(gfx.border_width()),
            config.theme.keyword,
        );

        gfx.add_text(
            self.mode.title,
            gfx.glyph_width(),
            gfx.border_width() + gfx.tab_padding_y(),
            config.theme.normal,
        );

        let doc_bounds = self.tab.doc_bounds();

        gfx.add_bordered_rect(
            doc_bounds
                .add_margin(gfx.border_width())
                .unoffset_by(self.widget.bounds()),
            SIDE_ALL,
            config.theme.background,
            config.theme.border,
        );

        gfx.end();

        self.tab.draw(None, &mut self.doc, config, gfx, is_focused);

        self.result_list.draw(config, gfx, |result| result);
    }

    pub fn open(
        &mut self,
        ui: &mut UiHandle,
        mode: &'static CommandPaletteMode,
        args: CommandPaletteEventArgs,
    ) {
        self.last_updated_version = None;
        self.mode = mode;
        self.widget.take_focus(ui);
        self.widget.is_visible = true;

        let on_open = self.mode.on_open;
        (on_open)(self, temp_args!(args));

        self.update_results(temp_args!(args));
    }

    fn close(&mut self, ui: &mut UiHandle, line_pool: &mut LinePool) {
        self.widget.is_visible = false;
        self.widget.release_focus(ui);
        self.doc.clear(line_pool);
    }
}
