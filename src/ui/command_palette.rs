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
    ui::{
        camera::{Camera, RECENTER_DISTANCE},
        tab::Tab,
    },
};

use file_mode::MODE_OPEN_FILE;
use mode::{CommandPaletteEventArgs, CommandPaletteMode};

use super::{doc_list::DocList, editor::Editor, pane::Pane};

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

    results: Vec<String>,
    selected_result_index: usize,
    previous_results: Vec<Line>,

    bounds: Rect,
    title_bounds: Rect,
    input_bounds: Rect,
    result_bounds: Rect,
    results_bounds: Rect,

    camera: Camera,
}

impl CommandPalette {
    pub fn new(line_pool: &mut LinePool) -> Self {
        Self {
            state: CommandPaletteState::Inactive,
            mode: MODE_OPEN_FILE,
            tab: Tab::new(0),
            doc: Doc::new(line_pool, DocKind::SingleLine),
            last_updated_version: None,

            results: Vec::new(),
            selected_result_index: 0,
            previous_results: Vec::new(),

            bounds: Rect::zero(),
            title_bounds: Rect::zero(),
            input_bounds: Rect::zero(),
            result_bounds: Rect::zero(),
            results_bounds: Rect::zero(),

            camera: Camera::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.state != CommandPaletteState::Inactive
    }

    pub fn is_animating(&self) -> bool {
        self.camera.is_moving()
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

        self.result_bounds = Rect::new(0.0, 0.0, self.input_bounds.width, gfx.line_height() * 1.25);

        self.results_bounds = Rect::new(
            0.0,
            0.0,
            self.input_bounds.width,
            self.result_bounds.height * self.results.len().min(MAX_VISIBLE_RESULTS) as f32,
        )
        .below(self.input_bounds)
        .shift_y(-gfx.border_width())
        .floor();

        self.bounds = self
            .title_bounds
            .expand_to_include(self.input_bounds)
            .expand_to_include(self.results_bounds)
            .center_x_in(bounds)
            .offset_by(Rect::new(0.0, gfx.tab_height() * 2.0, 0.0, 0.0))
            .floor();

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

        let mut handled_selected_result_index = self.selected_result_index;

        let mut mouse_handler = window.get_mousebind_handler();

        while let Some(mousebind) = mouse_handler.next(window) {
            let position = VisualPosition::new(mousebind.x, mousebind.y);

            let Mousebind {
                button: None | Some(MouseButton::Left),
                mods,
                ..
            } = mousebind
            else {
                continue;
            };

            if mousebind.button.is_some() && !self.bounds.contains_position(position) {
                self.close(line_pool);
                continue;
            }

            let results_bounds = self.results_bounds.offset_by(self.bounds);

            if !results_bounds.contains_position(position) {
                continue;
            }

            let clicked_result_index = ((position.y + self.camera.y() - results_bounds.y)
                / self.result_bounds.height) as usize;

            if clicked_result_index >= self.results.len() {
                continue;
            }

            self.selected_result_index = clicked_result_index;
            handled_selected_result_index = self.selected_result_index;

            if mousebind.button.is_some() {
                self.submit(
                    mods & MOD_SHIFT != 0,
                    pane,
                    doc_list,
                    config,
                    line_pool,
                    time,
                );
            }
        }

        let mut mouse_scroll_handler = window.get_mouse_scroll_handler();

        while let Some(mouse_scroll) = mouse_scroll_handler.next(window) {
            if mouse_scroll.is_horizontal {
                continue;
            }

            let position = VisualPosition::new(mouse_scroll.x, mouse_scroll.y);

            if self
                .results_bounds
                .offset_by(self.bounds)
                .contains_position(position)
            {
                let delta = mouse_scroll.delta * self.result_bounds.height;
                self.camera.vertical.scroll(delta);
            }
        }

        let mut keybind_handler = window.get_keybind_handler();

        while let Some(keybind) = keybind_handler.next(window) {
            match keybind {
                Keybind {
                    key: Key::Escape,
                    mods: 0,
                } => {
                    self.close(line_pool);
                }
                Keybind {
                    key: Key::Enter,
                    mods,
                } => {
                    self.submit(
                        mods & MOD_SHIFT != 0,
                        pane,
                        doc_list,
                        config,
                        line_pool,
                        time,
                    );
                }
                Keybind {
                    key: Key::Tab,
                    mods: 0,
                } => {
                    self.complete_result(pane, doc_list, config, line_pool, time);
                }
                Keybind {
                    key: Key::Up,
                    mods: 0,
                } => {
                    if self.selected_result_index > 0 {
                        self.selected_result_index -= 1;
                    }
                }
                Keybind {
                    key: Key::Down,
                    mods: 0,
                } => {
                    if self.selected_result_index < self.results.len() - 1 {
                        self.selected_result_index += 1;
                    }
                }
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

        self.tab.update(
            &mut self.doc,
            window,
            line_pool,
            text_buffer,
            config,
            time,
            dt,
        );

        window.clear_inputs();

        self.update_results(pane, doc_list, config, line_pool, time);

        self.selected_result_index = self
            .selected_result_index
            .clamp(0, self.results.len().saturating_sub(1));

        self.update_camera(handled_selected_result_index, dt);
    }

    fn update_camera(&mut self, handled_selected_result_index: usize, dt: f32) {
        let target_y =
            (self.selected_result_index as f32 + 0.5) * self.result_bounds.height - self.camera.y();
        let max_y = (self.results.len() as f32 * self.result_bounds.height
            - self.results_bounds.height)
            .max(0.0);

        let scroll_border_top = self.result_bounds.height * RECENTER_DISTANCE as f32;
        let scroll_border_bottom = self.results_bounds.height - scroll_border_top;

        let can_recenter = self.selected_result_index != handled_selected_result_index;

        self.camera.vertical.update(
            target_y,
            max_y,
            self.results_bounds.height,
            scroll_border_top,
            scroll_border_bottom,
            can_recenter,
            dt,
        );
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

        self.selected_result_index = 0;
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

        self.selected_result_index = 0;
        self.camera.reset();
        self.results.clear();

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
            self.results_bounds,
            SIDE_ALL,
            &config.theme.background,
            &config.theme.border,
        );

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

        gfx.begin(Some(self.results_bounds.offset_by(self.bounds)));

        let camera_y = self.camera.y().floor();

        let min_y = (camera_y / self.result_bounds.height) as usize;
        let sub_line_offset_y = camera_y - min_y as f32 * self.result_bounds.height;

        let max_y =
            ((camera_y + self.results_bounds.height) / self.result_bounds.height) as usize + 1;
        let max_result_index = max_y.min(self.results.len());

        for (i, y) in (min_y..max_result_index).enumerate() {
            let visual_y = i as f32 * self.result_bounds.height
                + (self.result_bounds.height - gfx.glyph_height()) / 2.0
                - sub_line_offset_y;

            let color = if y == self.selected_result_index {
                &config.theme.keyword
            } else {
                &config.theme.normal
            };

            let result = &self.results[y];
            gfx.add_text(result.chars(), gfx.glyph_width(), visual_y, color);
        }

        gfx.end();
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
