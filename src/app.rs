use std::path::{Path, PathBuf};

use crate::{
    config::{Config, ConfigError},
    ctx::Ctx,
    geometry::rect::Rect,
    lsp::Lsp,
    platform::{file_watcher::FileWatcher, gfx::Gfx, process::Process, window::Window},
    pool::Pooled,
    ui::{
        command_palette::CommandPalette,
        core::{Ui, WidgetId},
        editor::Editor,
        status_bar::StatusBar,
        terminal::Terminal,
    },
};

macro_rules! ctx_for_app {
    ($self:ident, $window:expr, $gfx:expr, $time:expr) => {
        &mut Ctx {
            window: $window,
            gfx: $gfx,
            ui: &mut $self.ui,
            config: &$self.config,
            lsp: &mut $self.lsp,
            time: $time,
        }
    };
}

pub struct App {
    ui: Ui,
    command_palette: CommandPalette,
    editor: Editor,
    terminal: Terminal,
    status_bar: StatusBar,

    file_watcher: FileWatcher,
    lsp: Lsp,

    config_dir: Pooled<PathBuf>,
    config: Config,
    config_error: Option<ConfigError>,
}

impl App {
    pub fn new(window: &mut Window, gfx: &mut Gfx, time: f32) -> Self {
        let config_dir = Config::dir();

        let (config, config_error) = match Config::load(&config_dir) {
            Ok(config) => (config, None),
            Err(err) => (Config::default(), Some(err)),
        };

        window.set_theme(&config.theme);
        gfx.set_font(&config.font, config.font_size);

        let mut ui = Ui::new();
        let mut lsp = Lsp::new();

        let mut ctx = Ctx {
            window,
            gfx,
            ui: &mut ui,
            config: &config,
            lsp: &mut lsp,
            time,
        };

        let mut app = Self {
            command_palette: CommandPalette::new(WidgetId::ROOT, ctx.ui),
            editor: Editor::new(WidgetId::ROOT, &mut ctx),
            terminal: Terminal::new(WidgetId::ROOT, &mut ctx),
            status_bar: StatusBar::new(WidgetId::ROOT, ctx.ui),
            ui,

            file_watcher: FileWatcher::new(),
            lsp,

            config_dir,
            config,
            config_error,
        };

        let (pane, _) = app.editor.last_focused_pane_and_doc_list(&app.ui);
        app.ui.focus(pane.widget_id());

        app.layout(window, gfx, time);
        app
    }

    pub fn update(&mut self, window: &mut Window, gfx: &mut Gfx, time: f32, dt: f32) {
        let config_changed = self
            .file_watcher
            .changed_files()
            .iter()
            .any(|changed_file| changed_file.starts_with(&self.config_dir));

        if config_changed {
            match Config::load(&self.config_dir) {
                Ok(config) => self.config = config,
                Err(err) => self.config_error = Some(err),
            }

            window.set_theme(&self.config.theme);
            gfx.set_font(&self.config.font, self.config.font_size);

            self.editor.clear_doc_highlights();
            self.layout(window, gfx, time);
        }

        if let Some(err) = window
            .was_shown()
            .then(|| self.config_error.take())
            .flatten()
        {
            err.show_message();
        }

        self.ui.update(window);

        let ctx = ctx_for_app!(self, window, gfx, time);

        Lsp::update(&mut self.editor, &mut self.command_palette, ctx);

        self.command_palette.update(&mut self.editor, ctx);
        self.editor.update(&mut self.file_watcher, ctx, dt);
        self.terminal.update(ctx);

        self.command_palette.update_camera(ctx, dt);
        self.editor.update_camera(ctx, dt);
        self.terminal.update_camera(ctx, dt);
    }

    pub fn draw(&mut self, window: &mut Window, gfx: &mut Gfx, time: f32) {
        self.layout(window, gfx, time);

        gfx.begin_frame(self.config.theme.background);

        let ctx = ctx_for_app!(self, window, gfx, time);

        self.status_bar.draw(&self.editor, ctx);
        self.terminal.draw(ctx);
        self.editor.draw(ctx);
        self.command_palette.draw(ctx);

        gfx.end_frame();
    }

    fn layout(&mut self, window: &mut Window, gfx: &mut Gfx, time: f32) {
        let mut bounds = Rect::new(0.0, 0.0, gfx.width(), gfx.height());
        self.ui.widget_mut(WidgetId::ROOT).bounds = bounds;

        let ctx = ctx_for_app!(self, window, gfx, time);

        self.command_palette.layout(bounds, ctx);

        self.status_bar.layout(bounds, ctx);
        let status_bar_bounds = ctx.ui.widget(self.status_bar.widget_id()).bounds;
        bounds = bounds.shrink_bottom_by(status_bar_bounds);

        self.terminal.layout(bounds, ctx);
        let terminal_bounds = ctx.ui.widget(self.terminal.widget_id()).bounds;
        bounds = bounds.shrink_bottom_by(terminal_bounds);

        self.editor.layout(bounds, ctx);
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
