use std::path::{Path, PathBuf};

use crate::{
    config::{Config, ConfigError},
    ctx::Ctx,
    geometry::rect::Rect,
    lsp::Lsp,
    platform::{file_watcher::FileWatcher, gfx::Gfx, process::Process, window::Window},
    pool::Pooled,
    ui::{
        command_palette::CommandPalette, core::Ui, editor::Editor, status_bar::StatusBar,
        terminal::Terminal,
    },
};

macro_rules! ctx_for_app {
    ($self:ident, $window:expr, $gfx:expr, $time:expr) => {
        &mut Ctx {
            window: $window,
            gfx: $gfx,
            config: &$self.config,
            lsp: &mut $self.lsp,
            time: $time,
        }
    };
}

pub struct App {
    ui: Ui,
    editor: Editor,
    terminal: Terminal,
    status_bar: StatusBar,
    command_palette: CommandPalette,

    file_watcher: FileWatcher,
    lsp: Lsp,

    config_dir: Pooled<PathBuf>,
    config: Config,
    config_error: Option<ConfigError>,
}

impl App {
    pub fn new() -> Self {
        let config_dir = Config::get_dir();

        let (config, config_error) = match Config::load(&config_dir) {
            Ok(config) => (config, None),
            Err(err) => (Config::default(), Some(err)),
        };

        let mut ui = Ui::new();

        Self {
            editor: Editor::new(&mut ui),
            terminal: Terminal::new(&mut ui),
            status_bar: StatusBar::new(&mut ui),
            command_palette: CommandPalette::new(&mut ui),
            ui,

            file_watcher: FileWatcher::new(),
            lsp: Lsp::new(),

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

        let ctx = ctx_for_app!(self, window, gfx, time);

        Lsp::update(
            &mut self.editor,
            &mut self.command_palette,
            &mut self.ui,
            ctx,
        );

        self.terminal.update(&mut self.ui, ctx);
        self.command_palette
            .update(&mut self.ui, &mut self.editor, ctx);
        self.editor
            .update(&mut self.ui, &mut self.file_watcher, ctx);

        self.layout(gfx);

        let ctx = ctx_for_app!(self, window, gfx, time);

        self.terminal.update_camera(&mut self.ui, ctx, dt);
        self.command_palette.update_camera(&mut self.ui, ctx, dt);
        self.editor.update_camera(&mut self.ui, ctx, dt);
    }

    pub fn draw(&mut self, window: &mut Window, gfx: &mut Gfx, time: f32) {
        self.layout(gfx);

        gfx.begin_frame(self.config.theme.background);

        let ctx = ctx_for_app!(self, window, gfx, time);

        self.status_bar.draw(&self.editor, ctx);
        self.terminal.draw(&mut self.ui, ctx);
        self.editor.draw(&mut self.ui, ctx);
        self.command_palette.draw(&mut self.ui, ctx);

        gfx.end_frame();
    }

    fn layout(&mut self, gfx: &mut Gfx) {
        let mut bounds = Rect::new(0.0, 0.0, gfx.width(), gfx.height());

        self.command_palette.layout(bounds, gfx);

        self.status_bar.layout(bounds, gfx);
        bounds = bounds.shrink_bottom_by(self.status_bar.widget.bounds());

        self.terminal.layout(bounds, &self.config, gfx);
        bounds = bounds.shrink_bottom_by(self.terminal.widget.bounds());

        self.editor.layout(bounds, gfx);
    }

    pub fn close(&mut self, window: &mut Window, gfx: &mut Gfx, time: f32) {
        let ctx = ctx_for_app!(self, window, gfx, time);

        self.editor.on_close(ctx);
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn is_animating(&self) -> bool {
        self.editor.is_animating()
            || self.terminal.is_animating()
            || self.command_palette.is_animating()
    }

    pub fn files_and_processes(
        &mut self,
    ) -> (
        &mut FileWatcher,
        impl Iterator<Item = &Path>,
        impl Iterator<Item = &mut Process>,
    ) {
        (
            &mut self.file_watcher,
            self.editor.files(),
            self.terminal.ptys().chain(self.lsp.processes()),
        )
    }
}
