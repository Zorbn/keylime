use crate::{
    geometry::position::Position,
    tests::{test_doc, HELLO_GOODBYE_TEXT},
    text::cursor_index::CursorIndex,
};

use super::editing_actions::{handle_delete_backward, DeleteKind};

test_doc!(
    delete_backward_wrap_to_previous_line,
    HELLO_GOODBYE_TEXT,
    |ctx, doc| {
        doc.jump_cursor(CursorIndex::Main, Position::new(0, 1), false, ctx.gfx);
        handle_delete_backward(DeleteKind::Char, doc, ctx);

        assert_eq!(
            doc.get_cursor(CursorIndex::Main).position,
            Position::new(11, 0)
        );

        assert_eq!(doc.get_line(0), Some("hello worldgoodbye world"));
        assert_eq!(doc.get_line(1), None);
    }
);
