use std::fmt::Write;

use crate::{
    config::Config,
    ctx::Ctx,
    geometry::{rect::Rect, sides::Sides},
    platform::gfx::Gfx,
    pool::{Pooled, STRING_POOL},
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
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        gfx.begin(Some(self.widget.bounds()));

        gfx.add_bordered_rect(
            self.widget.bounds().unoffset_by(self.widget.bounds()),
            Sides::ALL,
            theme.background,
            theme.border,
        );

        if let Some(status_text) = Self::get_status_text(editor, ctx.config) {
            let status_text_x = self.widget.bounds().width
                - (gfx.measure_text(&status_text) + 1) as f32 * gfx.glyph_width();
            let status_text_y = gfx.tab_padding_y();

            gfx.add_text(&status_text, status_text_x, status_text_y, theme.subtle);
        }

        gfx.end();
    }

    fn get_status_text(editor: &Editor, config: &Config) -> Option<Pooled<String>> {
        let (_, doc) = editor.get_focused_tab_and_doc()?;
        let position = doc.cursor(CursorIndex::Main).position;

        let mut status_text = STRING_POOL.new_item();

        if let Some(path) = doc
            .path()
            .some()
            .zip(editor.current_dir())
            .and_then(|(path, current_dir)| path.strip_prefix(current_dir).ok())
        {
            write!(&mut status_text, "{}, ", path.display()).ok()?;
        }

        if let Some(language) = config.get_language_for_doc(doc) {
            write!(&mut status_text, "{}, ", language.name).ok()?;
        }

        let line_ending_text = match doc.line_ending() {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF",
        };

        write!(
            &mut status_text,
            "{}, Ln {:02}, Col {:02}",
            line_ending_text,
            position.y + 1,
            position.x + 1
        )
        .ok()?;

        Some(status_text)
    }
}
