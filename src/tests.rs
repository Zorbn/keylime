macro_rules! test_with_doc {
    ($name:ident, $text:expr, $test:expr) => {
        #[test]
        fn $name() {
            let mut window = crate::platform::window::Window::new();
            let mut gfx = crate::platform::gfx::Gfx::new();
            let config = crate::config::Config::default();
            let mut buffers = crate::editor_buffers::EditorBuffers::new();
            let mut lsp = crate::lsp::Lsp::new();
            let time = 0.0;

            let ctx = &mut crate::ctx::Ctx {
                window: &mut window,
                gfx: &mut gfx,
                config: &config,
                buffers: &mut buffers,
                lsp: &mut lsp,
                time,
            };

            let mut doc = crate::text::doc::Doc::new(
                None,
                &mut ctx.buffers.lines,
                None,
                crate::text::doc::DocKind::MultiLine,
            );

            doc.insert(crate::geometry::position::Position::ZERO, $text, ctx);

            let test: fn(&mut crate::ctx::Ctx, &mut crate::text::doc::Doc) = $test;
            test(ctx, &mut doc);
        }
    };
}

pub(crate) use test_with_doc;

pub const HELLO_GOODBYE_TEXT: &str = r"hello world
goodbye world";
