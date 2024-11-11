use windows::{
    core::{w, Interface, Result},
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct2D::{
                Common::{D2D1_COLOR_F, D2D_RECT_F, D2D_SIZE_U},
                *,
            },
            DirectWrite::*,
            Dxgi::{Common::DXGI_FORMAT_UNKNOWN, IDXGISurface},
        },
    },
};
use Common::D2D1_ALPHA_MODE_PREMULTIPLIED;

const FONT_SIZE: f32 = 13.0;
const ATLAS_SIZE: usize = 256;

pub struct Text {
    dwrite_factory: IDWriteFactory,
    text_format: IDWriteTextFormat,

    d2d_factory: ID2D1Factory,
    //     d2d_hwnd_render_target: ID2D1HwndRenderTarget,
    //     black_brush: ID2D1SolidColorBrush,
    //
    //     width: i32,
    //     height: i32,
    glyph_width: f32,
    glyph_height: f32,
}

impl Text {
    pub unsafe fn new(/*hwnd: HWND, width: i32, height: i32*/) -> Result<Self> {
        let debug_level = if cfg!(debug_assertions) {
            D2D1_DEBUG_LEVEL_INFORMATION
        } else {
            D2D1_DEBUG_LEVEL_NONE
        };

        let d2d_factory: ID2D1Factory = D2D1CreateFactory(
            D2D1_FACTORY_TYPE_SINGLE_THREADED,
            Some(&D2D1_FACTORY_OPTIONS {
                debugLevel: debug_level,
            }),
        )?;
        let dwrite_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

        let text_format = dwrite_factory.CreateTextFormat(
            w!("Consolas"),
            None,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            FONT_SIZE,
            w!("en-us"),
        )?;

        let glyph_layout =
            dwrite_factory.CreateTextLayout(w!("M").as_wide(), &text_format, 1000.0, 1000.0)?;

        let mut glyph_metrics = DWRITE_TEXT_METRICS::default();
        glyph_layout.GetMetrics(&mut glyph_metrics)?;

        let glyph_width = glyph_metrics.width;
        let glyph_height = glyph_metrics.height;

        // let (d2d_hwnd_render_target, black_brush) =
        //     Self::create_resources(&d2d_factory, hwnd, width, height);

        Ok(Self {
            dwrite_factory,
            text_format,
            d2d_factory,
            // d2d_hwnd_render_target,
            // black_brush,

            // width,
            // height,
            glyph_width,
            glyph_height,
        })
    }

    pub fn atlas_size(&self) -> [f32; 2] {
        [self.glyph_width * ATLAS_SIZE as f32, self.glyph_height]
    }

    pub unsafe fn generate_atlas(&mut self, dxgi_surface: &IDXGISurface) {
        let render_target = self
            .d2d_factory
            .CreateDxgiSurfaceRenderTarget(
                dxgi_surface,
                &D2D1_RENDER_TARGET_PROPERTIES {
                    r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                    pixelFormat: Common::D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_UNKNOWN,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 96.0,
                    dpiY: 96.0,
                    ..Default::default()
                },
            )
            .unwrap();

        let white_brush = render_target
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                None,
            )
            .unwrap();

        render_target.BeginDraw();

        render_target.Clear(Some(&D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }));

        let [atlas_width, atlas_height] = self.atlas_size();

        let layout_rect = D2D_RECT_F {
            left: 0.0,
            top: 0.0,
            right: atlas_width,
            bottom: atlas_height,
        };

        render_target.DrawText(
            w!("Hello world").as_wide(),
            &self.text_format,
            &layout_rect,
            &white_brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );

        render_target.EndDraw(None, None).unwrap();
    }

    //     pub unsafe fn frame(&mut self, hwnd: HWND, width: i32, height: i32) {
    //         if (width != self.width || height != self.height) && width != 0 && height != 0 {
    //             (self.d2d_hwnd_render_target, self.black_brush) =
    //                 Self::create_resources(&self.d2d_factory, hwnd, width, height);
    //         }
    //
    //         self.d2d_hwnd_render_target.BeginDraw();
    //
    //         self.d2d_hwnd_render_target.Clear(Some(&D2D1_COLOR_F {
    //             r: 1.0,
    //             g: 1.0,
    //             b: 1.0,
    //             a: 1.0,
    //         }));
    //
    //         let layout_rect = D2D_RECT_F {
    //             left: 0.0,
    //             top: 0.0,
    //             right: 640.0,
    //             bottom: 480.0,
    //         };
    //
    //         self.d2d_hwnd_render_target.DrawText(
    //             w!("Hello world").as_wide(),
    //             &self.text_format,
    //             &layout_rect,
    //             &self.black_brush,
    //             D2D1_DRAW_TEXT_OPTIONS_NONE,
    //             DWRITE_MEASURING_MODE_NATURAL,
    //         );
    //
    //         self.d2d_hwnd_render_target.EndDraw(None, None).unwrap();
    //     }

    unsafe fn create_resources(
        d2d_factory: &ID2D1Factory,
        hwnd: HWND,
        width: i32,
        height: i32,
    ) -> (ID2D1HwndRenderTarget, ID2D1SolidColorBrush) {
        let d2d_hwnd_render_target = d2d_factory
            .CreateHwndRenderTarget(
                &D2D1_RENDER_TARGET_PROPERTIES::default(),
                &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd,
                    pixelSize: D2D_SIZE_U {
                        width: width as u32,
                        height: height as u32,
                    },
                    ..Default::default()
                },
            )
            .unwrap();

        let black_brush = d2d_hwnd_render_target
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                None,
            )
            .unwrap();

        (d2d_hwnd_render_target, black_brush)
    }
}
