macro_rules! test_with_doc {
    ($name:ident, $text:expr, $test:expr) => {
        #[test]
        fn $name() {
            let mut window = crate::platform::window::Window::new();
            let mut gfx = crate::platform::gfx::Gfx::new();
            let mut ui = crate::ui::core::Ui::new();
            let config = crate::config::Config::default();
            let mut lsp = crate::lsp::Lsp::new();
            let time = 0.0;

            let ctx = &mut crate::ctx::Ctx {
                window: &mut window,
                gfx: &mut gfx,
                ui: &mut ui,
                config: &config,
                lsp: &mut lsp,
                time,
            };

            let mut doc =
                crate::text::doc::Doc::new(None, None, crate::text::doc::DocFlags::MULTI_LINE);

            doc.insert(crate::geometry::position::Position::ZERO, $text, ctx);

            let test: fn(&mut crate::ctx::Ctx, &mut crate::text::doc::Doc) = $test;
            test(ctx, &mut doc);
        }
    };
}

pub(crate) use test_with_doc;

pub const HELLO_GOODBYE_TEXT: &str = r"hello world
goodbye world";
