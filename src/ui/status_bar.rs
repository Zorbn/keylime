use std::fmt::Write;

use crate::{
    config::Config,
    ctx::Ctx,
    geometry::{rect::Rect, sides::Sides},
    platform::gfx::Gfx,
    text::{cursor_index::CursorIndex, doc::LineEnding},
};

use super::{
    core::{Ui, Widget},
    editor::Editor,
};

pub struct StatusBar {
    pub widget: Widget,
}

impl StatusBar {
    pub fn new(ui: &mut Ui) -> Self {
        Self {
            widget: Widget::new(ui, true),
        }
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &mut Gfx) {
        let bounds = Rect::new(0.0, 0.0, bounds.width, gfx.tab_height())
            .at_bottom_of(bounds)
            .floor();

        self.widget.layout(&[bounds]);
    }

    pub fn draw(&mut self, editor: &Editor, ctx: &mut Ctx) {
        let status_text = Self::get_status_text(editor, ctx.config, ctx.buffers.text.get_mut())
            .unwrap_or_default();

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        let status_text_x = self.widget.bounds().width
            - (gfx.measure_text(status_text) + 1) as f32 * gfx.glyph_width();
        let status_text_y = gfx.tab_padding_y();

        gfx.begin(Some(self.widget.bounds()));

        gfx.add_bordered_rect(
            self.widget.bounds().unoffset_by(self.widget.bounds()),
            Sides::ALL,
            theme.background,
            theme.border,
        );

        gfx.add_text(status_text, status_text_x, status_text_y, theme.subtle);

        gfx.end();
    }

    fn get_status_text<'a>(
        editor: &Editor,
        config: &Config,
        text_buffer: &'a mut String,
    ) -> Option<&'a str> {
        let (_, doc) = editor.get_focused_tab_and_doc()?;
        let position = doc.get_cursor(CursorIndex::Main).position;

        if let Some(path) = doc
            .path()
            .some()
            .zip(editor.current_dir())
            .and_then(|(path, current_dir)| path.strip_prefix(current_dir).ok())
        {
            write!(text_buffer, "{}, ", path.display()).ok()?;
        }

        if let Some(language) = config.get_language_for_doc(doc) {
            write!(text_buffer, "{}, ", language.name).ok()?;
        }

        let line_ending_text = match doc.line_ending() {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF",
        };

        write!(
            text_buffer,
            "{}, Ln {:02}, Col {:02}",
            line_ending_text,
            position.y + 1,
            position.x + 1
        )
        .ok()?;

        Some(text_buffer)
    }
}
