use std::path::Path;

use crate::{
    config::{Config, ConfigError},
    editor_buffers::EditorBuffers,
    geometry::rect::Rect,
    platform::{pty::Pty, window::Window},
    temp_buffer::{TempBuffer, TempString},
    text::line_pool::LinePool,
    ui::{command_palette::CommandPalette, editor::Editor, terminal::Terminal, Ui},
};

pub struct App {
    buffers: EditorBuffers,

    ui: Ui,
    editor: Editor,
    terminal: Terminal,
    command_palette: CommandPalette,

    config: Config,
    config_error: Option<ConfigError>,
}

impl App {
    pub fn new() -> Self {
        let mut buffers = EditorBuffers {
            lines: LinePool::new(),
            cursors: TempBuffer::new(),
            text: TempString::new(),
        };

        let (config, config_error) = match Config::load() {
            Ok(config) => (config, None),
            Err(err) => (Config::default(), Some(err)),
        };

        let mut ui = Ui::new();
        let editor = Editor::new(&mut ui, &mut buffers.lines);
        let terminal = Terminal::new(&mut ui, &mut buffers.lines);
        let command_palette = CommandPalette::new(&mut ui, &mut buffers.lines);

        Self {
            buffers,

            ui,
            editor,
            terminal,
            command_palette,

            config,
            config_error,
        }
    }

    pub fn update(&mut self, window: &mut Window, timestamp: (f32, f32)) {
        if let Some(err) = window
            .was_shown()
            .then(|| self.config_error.take())
            .flatten()
        {
            err.show_message();
        }

        let mut ui = self.ui.get_handle(window);

        ui.update(&mut [
            &mut self.terminal.widget,
            &mut self.editor.widget,
            &mut self.command_palette.widget,
        ]);

        self.command_palette.update(
            &mut ui,
            &mut self.editor,
            &mut self.buffers,
            &self.config,
            timestamp,
        );

        self.terminal
            .update(&mut ui, &mut self.buffers, &self.config, timestamp);

        self.editor
            .update(&mut ui, &mut self.buffers, &self.config, timestamp);
    }

    pub fn draw(&mut self, window: &mut Window) {
        let mut ui = self.ui.get_handle(window);
        let gfx = ui.gfx();

        let mut bounds = Rect::new(0.0, 0.0, gfx.width(), gfx.height());

        self.command_palette.layout(bounds, gfx);
        self.terminal.layout(bounds, &self.config, gfx);

        bounds = bounds.shrink_bottom_by(self.terminal.bounds());

        self.editor.layout(bounds, gfx);

        gfx.begin_frame(self.config.theme.background);

        self.terminal.draw(&mut ui, &self.config);
        self.editor.draw(&mut ui, &self.config);
        self.command_palette.draw(&mut ui, &self.config);

        ui.gfx().end_frame();
    }

    pub fn close(&mut self, time: f32) {
        self.editor
            .on_close(&self.config, &mut self.buffers.lines, time);
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn is_dark(&self) -> bool {
        self.config.theme.is_dark()
    }

    pub fn is_animating(&self) -> bool {
        self.editor.is_animating()
            || self.terminal.is_animating()
            || self.command_palette.is_animating()
    }

    pub fn files_and_ptys(
        &mut self,
    ) -> (impl Iterator<Item = &Path>, impl Iterator<Item = &mut Pty>) {
        (self.editor.files(), self.terminal.ptys())
    }
}
