pub mod file_mode;
pub mod go_to_line_mode;
mod mode;
pub mod search_mode;

use crate::{
    config::Config,
    geometry::{
        rect::Rect,
        side::{SIDE_ALL, SIDE_LEFT, SIDE_RIGHT, SIDE_TOP},
        visual_position::VisualPosition,
    },
    input::{
        key::Key,
        keybind::{Keybind, MOD_CTRL, MOD_SHIFT},
        mouse_button::MouseButton,
        mousebind::Mousebind,
    },
    platform::{gfx::Gfx, window::Window},
    temp_buffer::TempBuffer,
    text::{
        doc::{Doc, DocKind},
        line_pool::{Line, LinePool},
    },
};

use super::{
    doc_list::DocList,
    editor::Editor,
    pane::Pane,
    result_list::{ResultList, ResultListInput},
    tab::Tab,
};

use file_mode::MODE_OPEN_FILE;
use mode::{CommandPaletteEventArgs, CommandPaletteMode};

#[derive(PartialEq, Eq)]
enum CommandPaletteState {
    Inactive,
    Active,
}

#[derive(Clone, Copy)]
pub enum CommandPaletteAction {
    Stay,
    Close,
    Open(&'static CommandPaletteMode),
}

const MAX_VISIBLE_RESULTS: usize = 20;

pub struct CommandPalette {
    state: CommandPaletteState,
    mode: &'static CommandPaletteMode,
    tab: Tab,
    doc: Doc,
    last_updated_version: Option<usize>,

    result_list: ResultList<String>,
    previous_results: Vec<Line>,

    bounds: Rect,
    title_bounds: Rect,
    input_bounds: Rect,
}

impl CommandPalette {
    pub fn new(line_pool: &mut LinePool) -> Self {
        Self {
            state: CommandPaletteState::Inactive,
            mode: MODE_OPEN_FILE,
            tab: Tab::new(0),
            doc: Doc::new(line_pool, DocKind::SingleLine),
            last_updated_version: None,

            result_list: ResultList::new(MAX_VISIBLE_RESULTS),
            previous_results: Vec::new(),

            bounds: Rect::zero(),
            title_bounds: Rect::zero(),
            input_bounds: Rect::zero(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.state != CommandPaletteState::Inactive
    }

    pub fn is_animating(&self) -> bool {
        self.result_list.is_animating()
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx) {
        let title = self.mode.title;
        let title_padding_x = gfx.glyph_width();
        let title_width =
            Gfx::measure_text(title.chars()) as f32 * gfx.glyph_width() + title_padding_x * 2.0;

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

        self.bounds = self
            .title_bounds
            .expand_to_include(self.input_bounds)
            .expand_to_include(self.result_list.bounds())
            .center_x_in(bounds)
            .offset_by(Rect::new(0.0, gfx.tab_height() * 2.0, 0.0, 0.0))
            .floor();

        self.result_list.offset_by(self.bounds);

        self.tab.layout(
            Rect::zero(),
            Rect::new(0.0, 0.0, gfx.glyph_width() * 10.0, gfx.line_height())
                .center_in(self.input_bounds)
                .expand_width_in(self.input_bounds)
                .offset_by(self.bounds)
                .floor(),
            &self.doc,
            gfx,
        );
    }

    pub fn update(
        &mut self,
        editor: &mut Editor,
        window: &mut Window,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
        dt: f32,
    ) {
        if !self.is_active() {
            return;
        }

        let (pane, doc_list) = editor.get_focused_pane_and_doc_list();

        let mut mouse_handler = window.get_mousebind_handler();

        while let Some(mousebind) = mouse_handler.next(window) {
            let position = VisualPosition::new(mousebind.x, mousebind.y);

            let Mousebind {
                button: None | Some(MouseButton::Left),
                ..
            } = mousebind
            else {
                mouse_handler.unprocessed(window, mousebind);
                continue;
            };

            if mousebind.button.is_some() && !self.bounds.contains_position(position) {
                self.close(line_pool);
                continue;
            }

            mouse_handler.unprocessed(window, mousebind);
        }

        let mut keybind_handler = window.get_keybind_handler();

        while let Some(keybind) = keybind_handler.next(window) {
            match keybind {
                Keybind {
                    key: Key::Backspace,
                    mods: 0 | MOD_CTRL,
                } => {
                    let on_backspace = self.mode.on_backspace;

                    let args = CommandPaletteEventArgs {
                        command_palette: self,
                        pane,
                        doc_list,
                        config,
                        line_pool,
                        time,
                    };

                    if !(on_backspace)(args) {
                        keybind_handler.unprocessed(window, keybind);
                    }
                }
                _ => keybind_handler.unprocessed(window, keybind),
            }
        }

        let result_input = self.result_list.update(window, true, true, dt);

        match result_input {
            ResultListInput::None => {}
            ResultListInput::Complete => {
                self.complete_result(pane, doc_list, config, line_pool, time)
            }
            ResultListInput::Submit { mods } => {
                self.submit(
                    mods & MOD_SHIFT != 0,
                    pane,
                    doc_list,
                    config,
                    line_pool,
                    time,
                );
            }
            ResultListInput::Close => self.close(line_pool),
        }

        self.tab
            .update(&mut self.doc, window, line_pool, text_buffer, config, time);

        window.clear_inputs();

        self.update_results(pane, doc_list, config, line_pool, time);
    }

    fn submit(
        &mut self,
        has_shift: bool,
        pane: &mut Pane,
        doc_list: &mut DocList,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        self.complete_result(pane, doc_list, config, line_pool, time);

        let on_submit = self.mode.on_submit;

        let args = CommandPaletteEventArgs {
            command_palette: self,
            pane,
            doc_list,
            config,
            line_pool,
            time,
        };

        let action = (on_submit)(args, has_shift);

        match action {
            CommandPaletteAction::Stay => {}
            CommandPaletteAction::Close | CommandPaletteAction::Open(_) => {
                if self.mode.do_passthrough_result {
                    for line in self.doc.drain(line_pool) {
                        self.previous_results.push(line);
                    }
                } else {
                    self.previous_results.clear();
                }

                self.close(line_pool);
            }
        }

        if let CommandPaletteAction::Open(mode) = action {
            self.open(mode, pane, doc_list, config, line_pool, time);
        }
    }

    fn complete_result(
        &mut self,
        pane: &mut Pane,
        doc_list: &mut DocList,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        let on_complete_result = self.mode.on_complete_result;

        let args = CommandPaletteEventArgs {
            command_palette: self,
            pane,
            doc_list,
            config,
            line_pool,
            time,
        };

        (on_complete_result)(args);

        self.result_list.drain();
    }

    fn update_results(
        &mut self,
        pane: &mut Pane,
        doc_list: &mut DocList,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        if Some(self.doc.version()) == self.last_updated_version {
            return;
        }

        self.last_updated_version = Some(self.doc.version());

        self.result_list.drain();

        let on_update_results = self.mode.on_update_results;

        let args = CommandPaletteEventArgs {
            command_palette: self,
            pane,
            doc_list,
            config,
            line_pool,
            time,
        };

        (on_update_results)(args);
    }

    pub fn draw(&mut self, config: &Config, gfx: &mut Gfx, is_focused: bool) {
        if !self.is_active() {
            return;
        }

        gfx.begin(Some(self.bounds));

        gfx.add_bordered_rect(
            self.input_bounds,
            SIDE_ALL,
            &config.theme.background,
            &config.theme.border,
        );

        gfx.add_bordered_rect(
            self.title_bounds,
            SIDE_LEFT | SIDE_RIGHT | SIDE_TOP,
            &config.theme.background,
            &config.theme.border,
        );

        gfx.add_rect(
            self.title_bounds.top_border(gfx.border_width()),
            &config.theme.keyword,
        );

        gfx.add_text(
            self.mode.title.chars(),
            gfx.glyph_width(),
            gfx.border_width() + gfx.tab_padding_y() + gfx.border_width(),
            &config.theme.normal,
        );

        let doc_bounds = self.tab.doc_bounds();

        gfx.add_bordered_rect(
            doc_bounds
                .add_margin(gfx.border_width())
                .unoffset_by(self.bounds),
            SIDE_ALL,
            &config.theme.background,
            &config.theme.border,
        );

        gfx.end();

        self.tab.draw(&mut self.doc, config, gfx, is_focused);

        self.result_list.draw(config, gfx, |result| result.chars());
    }

    pub fn open(
        &mut self,
        mode: &'static CommandPaletteMode,
        pane: &mut Pane,
        doc_list: &mut DocList,
        config: &Config,
        line_pool: &mut LinePool,
        time: f32,
    ) {
        self.last_updated_version = None;
        self.mode = mode;
        self.state = CommandPaletteState::Active;

        let on_open = self.mode.on_open;

        let args = CommandPaletteEventArgs {
            command_palette: self,
            pane,
            doc_list,
            config,
            line_pool,
            time,
        };

        (on_open)(args);

        self.update_results(pane, doc_list, config, line_pool, time);
    }

    fn close(&mut self, line_pool: &mut LinePool) {
        self.state = CommandPaletteState::Inactive;
        self.doc.clear(line_pool);
    }
}
