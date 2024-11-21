use crate::{
    config::Config,
    geometry::rect::Rect,
    platform::window::Window,
    temp_buffer::TempBuffer,
    text::{
        line_pool::LinePool,
        syntax_highlighter::{HighlightKind, Syntax, SyntaxRange},
    },
    ui::{command_palette::CommandPalette, editor::Editor},
};

pub struct App {
    line_pool: LinePool,
    text_buffer: TempBuffer<char>,
    command_palette: CommandPalette,
    editor: Editor,
    config: Config,
    syntax: Syntax,
}

impl App {
    pub fn new() -> Self {
        let mut line_pool = LinePool::new();
        let text_buffer = TempBuffer::new();

        let command_palette = CommandPalette::new(&mut line_pool);
        let editor = Editor::new(&mut line_pool);
        let config = Config::load().unwrap_or_default();

        let syntax = Syntax::new(
            &[
                "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false",
                "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
                "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
                "true", "type", "unsafe", "use", "where", "while",
            ],
            &[
                SyntaxRange {
                    start: "\"".into(),
                    end: "\"".into(),
                    escape: Some('\\'),
                    max_length: None,
                    kind: HighlightKind::String,
                },
                SyntaxRange {
                    start: "'".into(),
                    end: "'".into(),
                    escape: Some('\\'),
                    max_length: Some(1),
                    kind: HighlightKind::String,
                },
                SyntaxRange {
                    start: "#[".into(),
                    end: "]".into(),
                    escape: None,
                    max_length: None,
                    kind: HighlightKind::Preprocessor,
                },
                SyntaxRange {
                    start: "#![".into(),
                    end: "]".into(),
                    escape: None,
                    max_length: None,
                    kind: HighlightKind::Preprocessor,
                },
                SyntaxRange {
                    start: "//".into(),
                    end: "\n".into(),
                    escape: None,
                    max_length: None,
                    kind: HighlightKind::Comment,
                },
                SyntaxRange {
                    start: "/*".into(),
                    end: "*/".into(),
                    escape: None,
                    max_length: None,
                    kind: HighlightKind::Comment,
                },
            ],
        );

        Self {
            line_pool,
            text_buffer,
            command_palette,
            editor,
            config,
            syntax,
        }
    }

    pub fn update(&mut self, window: &mut Window) {
        let (time, dt) = window.update(self.is_animating());

        self.command_palette.update(
            &mut self.editor,
            window,
            &mut self.line_pool,
            &mut self.text_buffer,
            time,
            dt,
        );
        self.editor.update(
            &mut self.command_palette,
            window,
            &mut self.line_pool,
            &mut self.text_buffer,
            &self.syntax,
            time,
            dt,
        );
    }

    pub fn draw(&mut self, window: &mut Window) {
        let is_focused = window.is_focused();
        let gfx = window.gfx();
        let bounds = Rect::new(0.0, 0.0, gfx.width(), gfx.height());

        self.command_palette.layout(bounds, gfx);
        self.editor.layout(bounds, gfx);

        gfx.begin_frame(self.config.theme.background);

        self.editor.draw(
            &self.config.theme,
            gfx,
            is_focused && !self.command_palette.is_active(),
        );
        self.command_palette
            .draw(&self.config.theme, gfx, is_focused);

        gfx.end_frame();
    }

    pub fn close(&mut self) {
        self.editor.confirm_close_docs("exiting");
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn is_dark(&self) -> bool {
        self.config.theme.is_dark()
    }

    fn is_animating(&self) -> bool {
        self.editor.is_animating() || self.command_palette.is_animating()
    }
}
