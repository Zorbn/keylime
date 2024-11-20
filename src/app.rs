use crate::{
    command_palette::CommandPalette,
    editor::Editor,
    gfx::Color,
    line_pool::LinePool,
    rect::Rect,
    syntax_highlighter::{HighlightKind, Syntax, SyntaxRange},
    temp_buffer::TempBuffer,
    theme::Theme,
    window::Window,
};

pub struct App {
    line_pool: LinePool,
    text_buffer: TempBuffer<char>,
    command_palette: CommandPalette,
    editor: Editor,
    theme: Theme,
    syntax: Syntax,
}

impl App {
    pub fn new() -> Self {
        let mut line_pool = LinePool::new();
        let text_buffer = TempBuffer::new();

        let command_palette = CommandPalette::new(&mut line_pool);
        let editor = Editor::new(&mut line_pool);

        let theme = Theme {
            normal: Color::new(0, 0, 0, 255),
            comment: Color::new(0, 128, 0, 255),
            keyword: Color::new(0, 0, 255, 255),
            number: Color::new(9, 134, 88, 255),
            symbol: Color::new(0, 0, 0, 255),
            string: Color::new(163, 21, 21, 255),
            selection: Color::new(76, 173, 228, 125),
            border: Color::new(229, 229, 229, 255),
            background: Color::new(245, 245, 245, 255),
        };

        // let theme = Theme {
        //     normal: Color::new(204, 204, 204, 255),
        //     comment: Color::new(106, 153, 85, 255),
        //     keyword: Color::new(86, 156, 214, 255),
        //     number: Color::new(181, 206, 168, 255),
        //     symbol: Color::new(204, 204, 204, 255),
        //     string: Color::new(206, 145, 120, 255),
        //     selection: Color::new(76, 173, 228, 125),
        //     border: Color::new(43, 43, 43, 255),
        //     background: Color::new(30, 30, 30, 255),
        // };

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
            theme,
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

        gfx.begin_frame(self.theme.background);

        self.editor.draw(
            &self.theme,
            gfx,
            is_focused && !self.command_palette.is_active(),
        );
        self.command_palette.draw(&self.theme, gfx, is_focused);

        gfx.end_frame();
    }

    pub fn close(&mut self) {
        self.editor.confirm_close_docs("exiting");
    }

    pub fn is_dark(&self) -> bool {
        self.theme.is_dark()
    }

    fn is_animating(&self) -> bool {
        self.editor.is_animating() || self.command_palette.is_animating()
    }
}
