use crate::{
    config::Config,
    geometry::rect::Rect,
    platform::window::Window,
    temp_buffer::TempBuffer,
    text::line_pool::LinePool,
    ui::{command_palette::CommandPalette, editor::Editor, terminal::Terminal, Ui},
};

pub struct App {
    line_pool: LinePool,
    text_buffer: TempBuffer<char>,

    ui: Ui,
    editor: Editor,
    terminal: Terminal,
    command_palette: CommandPalette,

    config: Config,
}

impl App {
    pub fn new() -> Self {
        let mut line_pool = LinePool::new();
        let text_buffer = TempBuffer::new();

        let config = Config::load().unwrap_or_default();

        let mut ui = Ui::new();
        let editor = Editor::new(&mut ui, &mut line_pool);
        let terminal = Terminal::new(&mut ui, &mut line_pool);
        let command_palette = CommandPalette::new(&mut ui, &mut line_pool);

        Self {
            line_pool,
            text_buffer,

            ui,
            editor,
            terminal,
            command_palette,

            config,
        }
    }

    pub fn update(&mut self, window: &mut Window) {
        let (time, dt) = window.update(self.is_animating(), self.terminal.emulators());

        let mut ui = self.ui.get_handle(window);

        ui.update(&mut [
            &mut self.terminal.widget,
            &mut self.editor.widget,
            &mut self.command_palette.widget,
        ]);

        self.command_palette.update(
            &mut ui,
            &mut self.editor,
            &mut self.line_pool,
            &mut self.text_buffer,
            &self.config,
            time,
            dt,
        );

        self.terminal.update(
            &mut ui,
            &mut self.line_pool,
            &mut self.text_buffer,
            &self.config,
            time,
            dt,
        );

        self.editor.update(
            &mut ui,
            &mut self.line_pool,
            &mut self.text_buffer,
            &self.config,
            time,
            dt,
        );
    }

    pub fn draw(&mut self, window: &mut Window) {
        let mut ui = self.ui.get_handle(window);
        let gfx = ui.gfx();

        let mut bounds = Rect::new(0.0, 0.0, gfx.width(), gfx.height());

        self.command_palette.layout(bounds, gfx);
        self.terminal.layout(bounds, gfx);

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
}
