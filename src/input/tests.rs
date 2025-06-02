use crate::{
    geometry::position::Position,
    tests::{test_with_doc, HELLO_GOODBYE_TEXT},
    text::{cursor_index::CursorIndex, doc::DocFlags},
};

use super::editing_actions::{handle_delete_backward, handle_grapheme, DeleteKind};

test_with_doc!(
    delete_backward_wrap_to_previous_line,
    HELLO_GOODBYE_TEXT,
    |ctx, doc| {
        doc.jump_cursor(CursorIndex::Main, Position::new(0, 1), false, ctx.gfx);
        handle_delete_backward(DeleteKind::Char, doc, ctx);

        assert_eq!(doc.cursor(CursorIndex::Main).position, Position::new(11, 0));
        assert_eq!(doc.get_line(0), Some("hello worldgoodbye world"));
        assert_eq!(doc.get_line(1), None);
    }
);

test_with_doc!(match_pairs_in_multi_line_doc, "run_app", |ctx, doc| {
    handle_grapheme("(", doc, ctx);

    assert_eq!(doc.to_string(), "run_app()");
    assert_eq!(doc.cursor(CursorIndex::Main).position, Position::new(8, 0));
});

test_with_doc!(
    dont_match_pairs_in_single_line_doc,
    "run_app",
    DocFlags::SINGLE_LINE,
    |ctx, doc| {
        handle_grapheme("(", doc, ctx);

        assert_eq!(doc.to_string(), "run_app(");
        assert_eq!(doc.cursor(CursorIndex::Main).position, Position::new(8, 0));
    }
);

test_with_doc!(dont_match_apostrophe, "that", |ctx, doc| {
    handle_grapheme("'", doc, ctx);

    assert_eq!(doc.to_string(), "that'");
});

test_with_doc!(dont_match_lifetime_quote, "hello<", |ctx, doc| {
    handle_grapheme("'", doc, ctx);

    assert_eq!(doc.to_string(), "hello<'");
});

test_with_doc!(match_single_quote, "let a = ", |ctx, doc| {
    handle_grapheme("'", doc, ctx);

    assert_eq!(doc.to_string(), "let a = ''");
    assert_eq!(doc.cursor(CursorIndex::Main).position, Position::new(9, 0));
});

test_with_doc!(dont_match_closing_double_quote, "\"hello", |ctx, doc| {
    handle_grapheme("\"", doc, ctx);

    assert_eq!(doc.to_string(), "\"hello\"");
});

test_with_doc!(match_opening_double_quote, "let a = ", |ctx, doc| {
    handle_grapheme("\"", doc, ctx);

    assert_eq!(doc.to_string(), "let a = \"\"");
    assert_eq!(doc.cursor(CursorIndex::Main).position, Position::new(9, 0));
});

test_with_doc!(surround_selection_with_pair, "hi there", |ctx, doc| {
    for (i, grapheme) in ["\"", "'", "(", "{", "["].iter().enumerate() {
        let start = Position::ZERO;
        let end = doc.end();

        let (start, end) = if i % 2 == 0 {
            (start, end)
        } else {
            (end, start)
        };

        doc.jump_cursor(CursorIndex::Main, start, false, ctx.gfx);
        doc.jump_cursor(CursorIndex::Main, end, true, ctx.gfx);
        handle_grapheme(grapheme, doc, ctx);
    }

    assert_eq!(doc.to_string(), "[{('\"hi there\"')}]");
});
