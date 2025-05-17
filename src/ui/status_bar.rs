use std::{cmp::Ordering, fmt::Write};

use crate::{
    config::Config,
    ctx::Ctx,
    geometry::sides::Sides,
    lsp::{types::DecodedDiagnostic, Lsp},
    pool::{format_pooled, Pooled, STRING_POOL},
    text::{cursor_index::CursorIndex, doc::LineEnding},
};

use super::{
    core::{Ui, WidgetId, WidgetLayout},
    editor::Editor,
};

pub fn status_bar(editor: &Editor, ctx: &mut Ctx) {
    let gfx = &mut ctx.gfx;
    let theme = &ctx.config.theme;

    ctx.ui.begin_widget(
        WidgetId::Name("StatusBar"),
        WidgetLayout {
            height: Some(gfx.tab_height()),
            ..Default::default()
        },
        gfx,
    );

    gfx.add_bordered_rect(
        ctx.ui.bounds().unoffset_by(ctx.ui.bounds()),
        Sides::ALL,
        theme.background,
        theme.border,
    );

    let mut text_x = ctx.ui.bounds().width;
    let text_y = gfx.tab_padding_y();

    if let Some(text) = get_doc_text(editor, ctx.config, ctx.ui) {
        text_x -= gfx.measure_text(&text) as f32 * gfx.glyph_width();
        gfx.add_text(&text, text_x, text_y, theme.subtle);
    }

    if let Some((text, severity)) = get_problems_text(ctx.lsp) {
        let color = DecodedDiagnostic::severity_color(severity, theme);
        let separator = ", ";

        text_x -= gfx.measure_text(separator) as f32 * gfx.glyph_width();
        gfx.add_text(separator, text_x, text_y, theme.subtle);

        text_x -= gfx.measure_text(&text) as f32 * gfx.glyph_width();
        gfx.add_text(&text, text_x, text_y, color);
    }

    ctx.ui.end_widget(gfx);
}

fn get_problems_text(lsp: &mut Lsp) -> Option<(Pooled<String>, usize)> {
    let mut count = 0;
    let mut severity = usize::MAX;

    for server in lsp.iter_servers_mut() {
        for (_, diagnostics) in server.all_diagnostics_mut() {
            for diagnostic in diagnostics.encoded() {
                if !diagnostic.is_problem() {
                    continue;
                }

                severity = severity.min(diagnostic.severity);
                count += 1;
            }

            for diagnostic in diagnostics.decoded() {
                if !diagnostic.is_problem() {
                    continue;
                }

                severity = severity.min(diagnostic.severity);
                count += 1;
            }
        }
    }

    let text = match count.cmp(&1) {
        Ordering::Equal => format_pooled!("{} Problem", count),
        Ordering::Greater => format_pooled!("{} Problems", count),
        _ => return None,
    };

    Some((text, severity))
}

fn get_doc_text(editor: &Editor, config: &Config, ui: &Ui) -> Option<Pooled<String>> {
    let (pane, doc_list) = editor.last_focused_pane_and_doc_list(ui);
    let (_, doc) = pane.get_focused_tab_with_data(doc_list)?;
    let position = doc.cursor(CursorIndex::Main).position;

    let mut doc_text = STRING_POOL.new_item();

    if let Some(path) = doc
        .path()
        .some()
        .zip(editor.current_dir())
        .and_then(|(path, current_dir)| path.strip_prefix(current_dir).ok())
    {
        write!(&mut doc_text, "{}, ", path.display()).ok()?;
    }

    if let Some(language) = config.get_language_for_doc(doc) {
        write!(&mut doc_text, "{}, ", language.name).ok()?;
    }

    let line_ending_text = match doc.line_ending() {
        LineEnding::Lf => "LF",
        LineEnding::CrLf => "CRLF",
    };

    write!(
        &mut doc_text,
        "{}, Ln {:02}, Col {:02} ",
        line_ending_text,
        position.y + 1,
        position.x + 1
    )
    .ok()?;

    Some(doc_text)
}
