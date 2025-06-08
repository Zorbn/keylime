use crate::{
    ctx::ctx_with_time,
    geometry::position::Position,
    tests::{test_with_doc, HELLO_GOODBYE_TEXT},
    text::{action_history::ActionKind, cursor_index::CursorIndex},
};

test_with_doc!(search_forward, HELLO_GOODBYE_TEXT, |ctx, doc| {
    let position = doc.search("world", doc.line_end(0), false, ctx.gfx);
    assert_eq!(position, Some(Position::new(8, 1)));
});

test_with_doc!(search_forward_wrap, HELLO_GOODBYE_TEXT, |ctx, doc| {
    let position = doc.search("hello", Position::new(0, 1), false, ctx.gfx);
    assert_eq!(position, Some(Position::new(0, 0)));
});

test_with_doc!(
    search_forward_wrap_disabled,
    HELLO_GOODBYE_TEXT,
    |ctx, doc| {
        let position = doc.search_forward("hello", Position::new(0, 1), false, ctx.gfx);
        assert_eq!(position, None);
    }
);

test_with_doc!(search_backward, HELLO_GOODBYE_TEXT, |ctx, doc| {
    let position = doc.search("world", Position::new(0, 1), true, ctx.gfx);
    assert_eq!(position, Some(Position::new(6, 0)));
});

test_with_doc!(search_backward_wrap, HELLO_GOODBYE_TEXT, |ctx, doc| {
    let position = doc.search("goodbye", Position::new(0, 1), true, ctx.gfx);
    assert_eq!(position, Some(Position::new(0, 1)));
});

test_with_doc!(
    search_backward_wrap_disabled,
    HELLO_GOODBYE_TEXT,
    |ctx, doc| {
        let position = doc.search_backward("goodbye", Position::new(0, 1), false, ctx.gfx);
        assert_eq!(position, None);
    }
);

test_with_doc!(repeated_search_forward, HELLO_GOODBYE_TEXT, |ctx, doc| {
    let position = doc.search("world", Position::ZERO, false, ctx.gfx);
    assert_eq!(position, Some(Position::new(6, 0)));

    let position = doc.search("world", position.unwrap(), false, ctx.gfx);
    assert_eq!(position, Some(Position::new(8, 1)));

    let position = doc.search("world", position.unwrap(), false, ctx.gfx);
    assert_eq!(position, Some(Position::new(6, 0)));
});

test_with_doc!(repeated_search_backward, HELLO_GOODBYE_TEXT, |ctx, doc| {
    let position = doc.search("world", Position::ZERO, true, ctx.gfx);
    assert_eq!(position, Some(Position::new(8, 1)));

    let position = doc.search("world", position.unwrap(), true, ctx.gfx);
    assert_eq!(position, Some(Position::new(6, 0)));

    let position = doc.search("world", position.unwrap(), true, ctx.gfx);
    assert_eq!(position, Some(Position::new(8, 1)));
});

test_with_doc!(select_next_occurances, HELLO_GOODBYE_TEXT, |ctx, doc| {
    doc.jump_cursor(CursorIndex::Main, Position::new(6, 0), false, ctx.gfx);

    doc.add_cursor_at_next_occurance(ctx.gfx);
    assert_eq!(doc.cursor(CursorIndex::Main).position, Position::new(11, 0));
    assert_eq!(doc.cursors_len(), 1);

    doc.add_cursor_at_next_occurance(ctx.gfx);
    assert_eq!(doc.cursor(CursorIndex::Main).position, Position::new(13, 1));
    assert_eq!(doc.cursors_len(), 2);
});

test_with_doc!(
    select_next_occurance_from_selection,
    HELLO_GOODBYE_TEXT,
    |ctx, doc| {
        doc.jump_cursor(CursorIndex::Main, Position::new(11, 0), false, ctx.gfx);
        doc.jump_cursor(CursorIndex::Main, Position::new(6, 0), true, ctx.gfx);

        doc.add_cursor_at_next_occurance(ctx.gfx);
        assert_eq!(doc.cursor(CursorIndex::Main).position, Position::new(13, 1));
        assert_eq!(doc.cursors_len(), 2);
    }
);

test_with_doc!(multi_cursor_undo, HELLO_GOODBYE_TEXT, |ctx, doc| {
    let ctx = ctx_with_time!(ctx, 1.0);

    doc.jump_cursor(CursorIndex::Main, Position::ZERO, false, ctx.gfx);
    doc.add_cursor_at(Position::new(0, 1), ctx.gfx);
    doc.insert_at_cursors("test", ctx);

    assert_eq!(doc.to_string(), "testhello world\ntestgoodbye world");

    doc.clear_extra_cursors(CursorIndex::Main);
    doc.undo(ActionKind::Done, ctx);

    assert_eq!(doc.to_string(), "hello world\ngoodbye world");
    assert_eq!(doc.cursor(CursorIndex::Main).position, Position::new(0, 1));
    assert_eq!(doc.cursors_len(), 2);
});
