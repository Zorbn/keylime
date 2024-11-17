#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

mod action_history;
mod char_category;
mod cursor;
mod cursor_index;
mod deferred_call;
mod dialog;
mod doc;
mod editor;
mod gfx;
mod input_handlers;
mod key;
mod keybind;
mod line_pool;
mod matrix;
mod mouse_button;
mod mouse_scroll;
mod mousebind;
mod position;
mod rect;
mod selection;
mod syntax_highlighter;
mod tab;
mod temp_buffer;
mod text;
mod theme;
mod visual_position;
mod window;

use editor::Editor;
use gfx::Color;
use line_pool::LinePool;
use rect::Rect;
use syntax_highlighter::{HighlightKind, Syntax, SyntaxRange};
use temp_buffer::TempBuffer;
use theme::Theme;
use window::Window;

/*
 * TODO:
 * Multiple panes (split view).
 * File tree.
 * Per file type indentation.
 * Comment region: ctrl-/.
 * Indent-unindent region: ctrl-[, ctrl-], tab, shift-tab.
 * Support for OS scaling (eg. 125% applied for a monitor).
 * Search and search & replace.
 * Running commands and seeing output (very simple integrated terminal).
 * Find in files.
 * Configuration file: colors, fonts.
 * Command palette (eg. for ctrl-g, go to line).
 * Simple auto-complete.
 */

fn main() {
    println!("Hello, world!");

    let mut line_pool = LinePool::new();
    let mut text_buffer = TempBuffer::new();

    let mut editor = Editor::new(&mut line_pool);

    let mut window = Window::new().unwrap();

    let theme = Theme {
        normal: Color::new(0, 0, 0, 255),
        comment: Color::new(0, 128, 0, 255),
        keyword: Color::new(0, 0, 255, 255),
        number: Color::new(9, 134, 88, 255),
        symbol: Color::new(0, 0, 0, 255),
        string: Color::new(163, 21, 21, 255),
        border: Color::new(229, 229, 229, 255),
        background: Color::new(245, 245, 245, 255),
    };

    let syntax = Syntax::new(
        &[
            "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
            "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
            "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
            "unsafe", "use", "where", "while",
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

    while window.is_running() {
        let (time, dt) = window.update(editor.is_animating());

        editor.update(
            &mut window,
            &mut line_pool,
            &mut text_buffer,
            &syntax,
            time,
            dt,
        );

        let is_focused = window.is_focused();
        let gfx = window.gfx();
        let bounds = Rect::new(0.0, 0.0, gfx.width(), gfx.height());

        editor.layout(bounds, gfx);

        gfx.begin_frame(theme.background);

        editor.draw(&theme, gfx, is_focused);

        gfx.end_frame();
    }

    editor.confirm_close_docs("exiting");
}
