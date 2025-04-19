use std::path::{Path, PathBuf};

use crate::{
    config::{Config, ConfigError},
    ctx::Ctx,
    editor_buffers::EditorBuffers,
    geometry::rect::Rect,
    platform::{file_watcher::FileWatcher, gfx::Gfx, pty::Pty, window::Window},
    temp_buffer::{TempBuffer, TempString},
    text::line_pool::LinePool,
    ui::{command_palette::CommandPalette, core::Ui, editor::Editor, terminal::Terminal},
};

pub struct App {
    buffers: EditorBuffers,

    ui: Ui,
    editor: Editor,
    terminal: Terminal,
    command_palette: CommandPalette,
    file_watcher: FileWatcher,

    config_dir: PathBuf,
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

        let config_dir = Config::get_dir();

        let (config, config_error) = match Config::load(&config_dir) {
            Ok(config) => (config, None),
            Err(err) => (Config::default(), Some(err)),
        };

        let mut ui = Ui::new();
        let editor = Editor::new(&mut ui, &mut buffers.lines);
        let terminal = Terminal::new(&mut ui, &mut buffers.lines);
        let command_palette = CommandPalette::new(&mut ui, &mut buffers.lines);
        let file_watcher = FileWatcher::new();

        Self {
            buffers,

            ui,
            editor,
            terminal,
            command_palette,
            file_watcher,

            config_dir,
            config,
            config_error,
        }
    }

    pub fn update(&mut self, window: &mut Window, gfx: &mut Gfx, time: f32, dt: f32) {
        let config_changed = self
            .file_watcher
            .get_changed_files()
            .iter()
            .any(|changed_file| changed_file.starts_with(&self.config_dir));

        if config_changed {
            match Config::load(&self.config_dir) {
                Ok(config) => self.config = config,
                Err(err) => self.config_error = Some(err),
            }

            window.set_theme(&self.config.theme);
            gfx.update_font(&self.config.font, self.config.font_size);

            self.editor.clear_doc_highlights();
            self.layout(gfx);
        }

        if let Some(err) = window
            .was_shown()
            .then(|| self.config_error.take())
            .flatten()
        {
            err.show_message();
        }

        self.ui.update(
            &mut [
                &mut self.terminal.widget,
                &mut self.editor.widget,
                &mut self.command_palette.widget,
            ],
            window,
        );

        let mut ctx = Ctx {
            window,
            gfx,
            config: &self.config,
            buffers: &mut self.buffers,
            time,
        };

        self.terminal.update(&mut self.ui, &mut ctx, dt);

        self.command_palette
            .update(&mut self.ui, &mut self.editor, &mut ctx, dt);

        self.editor
            .update(&mut self.ui, &mut self.file_watcher, &mut ctx, dt);
    }

    pub fn draw(&mut self, window: &mut Window, gfx: &mut Gfx, time: f32) {
        self.layout(gfx);

        gfx.begin_frame(self.config.theme.background);

        let mut ctx = Ctx {
            window,
            gfx,
            config: &self.config,
            buffers: &mut self.buffers,
            time,
        };

        self.terminal.draw(&mut self.ui, &mut ctx);
        self.editor.draw(&mut self.ui, &mut ctx);
        self.command_palette.draw(&mut self.ui, &mut ctx);

        gfx.end_frame();
    }

    fn layout(&mut self, gfx: &mut Gfx) {
        let mut bounds = Rect::new(0.0, 0.0, gfx.width(), gfx.height());

        self.command_palette.layout(bounds, gfx);
        self.terminal.layout(bounds, &self.config, gfx);

        bounds = bounds.shrink_bottom_by(self.terminal.bounds());

        self.editor.layout(bounds, gfx);
    }

    pub fn close(&mut self, window: &mut Window, gfx: &mut Gfx, time: f32) {
        let mut ctx = Ctx {
            window,
            gfx,
            config: &self.config,
            buffers: &mut self.buffers,
            time,
        };

        self.editor.on_close(&mut ctx);
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn is_animating(&self) -> bool {
        self.editor.is_animating()
            || self.terminal.is_animating()
            || self.command_palette.is_animating()
    }

    pub fn files_and_ptys(
        &mut self,
    ) -> (
        &mut FileWatcher,
        impl Iterator<Item = &Path>,
        impl Iterator<Item = &mut Pty>,
    ) {
        (
            &mut self.file_watcher,
            self.editor.files(),
            self.terminal.ptys(),
        )
    }
}
