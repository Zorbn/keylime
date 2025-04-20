use crate::{
    geometry::position::Position,
    tests::{test_with_doc, HELLO_GOODBYE_TEXT},
    text::pattern::PatternMatch,
};

use super::pattern::Pattern;

test_with_doc!(search_forward, HELLO_GOODBYE_TEXT, |ctx, doc| {
    let position = doc.search("world", doc.get_line_end(0), false, ctx.gfx);
    assert_eq!(position, Some(Position::new(8, 1)));
});

test_with_doc!(search_forward_wrap, HELLO_GOODBYE_TEXT, |ctx, doc| {
    let position = doc.search("hello", Position::new(0, 1), false, ctx.gfx);
    assert_eq!(position, Some(Position::new(0, 0)));
});

test_with_doc!(
    search_forward_wrap_disabled,
    HELLO_GOODBYE_TEXT,
    |_, doc| {
        let position = doc.search_forward("hello", Position::new(0, 1), false);
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

#[test]
fn match_html_open_tag() -> Result<(), &'static str> {
    let pattern = Pattern::parse("<(%w+)>".into())?;

    assert_eq!(
        pattern.match_text("<body>", 0),
        Some(PatternMatch { start: 1, end: 5 })
    );

    assert_eq!(pattern.match_text("<body", 0), None);
    assert_eq!(pattern.match_text("<>", 0), None);

    Ok(())
}

#[test]
fn match_simple_class() -> Result<(), &'static str> {
    let pattern = Pattern::parse("[abcdef]+".into())?;

    assert_eq!(
        pattern.match_text("fdcbbzyx", 0),
        Some(PatternMatch { start: 0, end: 5 })
    );

    assert_eq!(pattern.match_text("!", 0), None);

    assert_eq!(
        pattern.match_text("a", 0),
        Some(PatternMatch { start: 0, end: 1 })
    );

    Ok(())
}

#[test]
fn match_hex_number() -> Result<(), &'static str> {
    let pattern = Pattern::parse("0x%x+".into())?;

    assert_eq!(
        pattern.match_text("0xC0FFEE", 0),
        Some(PatternMatch { start: 0, end: 8 })
    );

    assert_eq!(
        pattern.match_text("0xc0ffee", 0),
        Some(PatternMatch { start: 0, end: 8 })
    );

    assert_eq!(pattern.match_text("0xNOTHEX", 0), None);

    Ok(())
}

#[test]
fn match_comment() -> Result<(), &'static str> {
    let pattern = Pattern::parse("//%.*".into())?;

    assert_eq!(
        pattern.match_text("// this is a comment // still the same comment", 0),
        Some(PatternMatch { start: 0, end: 46 })
    );

    assert_eq!(
        pattern.match_text("// skipping this comment // not the same comment", 25),
        Some(PatternMatch { start: 25, end: 48 })
    );

    assert_eq!(pattern.match_text("/* not the right comment */", 0), None);

    Ok(())
}

#[test]
fn match_unicode_escape_sequence() -> Result<(), &'static str> {
    let pattern = Pattern::parse("'\\u{%x-}'".into())?;

    assert_eq!(
        pattern.match_text("'\\u{ABCD}' '\\u{EF01}'", 0),
        Some(PatternMatch { start: 0, end: 10 })
    );

    assert_eq!(pattern.match_text("'\\u{GHIJ}'", 0), None);

    Ok(())
}

#[test]
fn match_float() -> Result<(), &'static str> {
    let pattern = Pattern::parse("(%d[%d_]*.?[%d_]*)[^.]".into())?;

    assert_eq!(
        pattern.match_text("0.1", 0),
        Some(PatternMatch { start: 0, end: 3 })
    );

    assert_eq!(
        pattern.match_text("123.456", 0),
        Some(PatternMatch { start: 0, end: 7 })
    );

    assert_eq!(
        pattern.match_text("123.", 0),
        Some(PatternMatch { start: 0, end: 4 })
    );

    assert_eq!(pattern.match_text("A12.", 0), None);
    assert_eq!(pattern.match_text(".", 0), None);
    assert_eq!(pattern.match_text("0..1", 0), None);

    Ok(())
}

#[test]
fn match_negative_class() -> Result<(), &'static str> {
    let pattern = Pattern::parse("[^abc]+".into())?;

    assert_eq!(
        pattern.match_text("def", 0),
        Some(PatternMatch { start: 0, end: 3 })
    );

    assert_eq!(
        pattern.match_text("hello c b a", 0),
        Some(PatternMatch { start: 0, end: 6 })
    );

    assert_eq!(
        pattern.match_text("def^", 0),
        Some(PatternMatch { start: 0, end: 4 })
    );

    assert_eq!(pattern.match_text("a", 0), None);

    Ok(())
}
