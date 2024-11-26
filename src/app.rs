use crate::{
    config::Config,
    geometry::rect::Rect,
    platform::{pty::Pty, window::Window},
    temp_buffer::TempBuffer,
    text::line_pool::LinePool,
    ui::{command_palette::CommandPalette, editor::Editor, terminal::Terminal},
};

pub struct App {
    line_pool: LinePool,
    text_buffer: TempBuffer<char>,
    command_palette: CommandPalette,
    terminal: Terminal,
    editor: Editor,
    config: Config,
}

impl App {
    pub fn new() -> Self {
        let mut line_pool = LinePool::new();
        let text_buffer = TempBuffer::new();

        let config = Config::load().unwrap_or_default();

        let command_palette = CommandPalette::new(&mut line_pool);
        let terminal = Terminal::new(&mut line_pool);
        let editor = Editor::new(&config, &mut line_pool, 0.0);

        Self {
            line_pool,
            text_buffer,
            command_palette,
            terminal,
            editor,
            config,
        }
    }

    pub fn update(&mut self, window: &mut Window) {
        let (time, dt) = window.update(self.is_animating(), self.pty());

        self.command_palette.update(
            &mut self.editor,
            window,
            &mut self.line_pool,
            &mut self.text_buffer,
            &self.config,
            time,
            dt,
        );

        self.terminal.update(
            window,
            &mut self.line_pool,
            &mut self.text_buffer,
            &self.config,
            time,
            dt,
        );

        self.editor.update(
            &mut self.command_palette,
            window,
            &mut self.line_pool,
            &mut self.text_buffer,
            &self.config,
            time,
            dt,
        );
    }

    pub fn draw(&mut self, window: &mut Window) {
        let is_focused = window.is_focused();
        let gfx = window.gfx();
        let mut bounds = Rect::new(0.0, 0.0, gfx.width(), gfx.height());

        self.command_palette.layout(bounds, gfx);
        self.terminal.layout(bounds, gfx);

        bounds = bounds.shrink_bottom_by(self.terminal.bounds());

        self.editor.layout(bounds, gfx);

        gfx.begin_frame(self.config.theme.background);

        self.editor.draw(
            &self.config,
            gfx,
            // TODO: Editor and terminal should not be focused at once.
            is_focused && !self.command_palette.is_active(),
        );
        self.terminal.draw(
            &self.config,
            gfx,
            // TODO: Editor and terminal should not be focused at once.
            is_focused && !self.command_palette.is_active(),
        );
        self.command_palette.draw(&self.config, gfx, is_focused);

        gfx.end_frame();
    }

    pub fn close(&mut self, time: f32) {
        self.editor
            .on_close(&self.config, &mut self.line_pool, time);

        self.terminal.on_close();
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn is_dark(&self) -> bool {
        self.config.theme.is_dark()
    }

    fn is_animating(&self) -> bool {
        self.editor.is_animating()
            || self.terminal.is_animating()
            || self.command_palette.is_animating()
    }

    fn pty(&self) -> Option<&Pty> {
        self.terminal.pty()
    }
}
