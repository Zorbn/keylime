use crate::{
    geometry::position::Position,
    tests::{test_with_doc, HELLO_GOODBYE_TEXT},
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
