use std::{ffi::c_void, ptr::null, slice::from_raw_parts};

use windows::{
    core::{implement, w, Error, Interface, Result, HSTRING, PCWSTR},
    Win32::{
        Foundation::{DWRITE_E_NOCOLOR, E_FAIL, FALSE, TRUE},
        Graphics::{
            Direct2D::{
                Common::{D2D1_COLOR_F, D2D1_PIXEL_FORMAT},
                *,
            },
            DirectWrite::*,
            Dxgi::Common::DXGI_FORMAT_UNKNOWN,
            Imaging::{
                CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGR,
                GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory, WICBitmapCacheOnDemand,
            },
        },
        System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
    },
};
use windows_core::{IUnknown, Ref, BOOL};
use windows_numerics::{Matrix3x2, Vector2};
use Common::{D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED};

use crate::{
    platform::{
        text::GlyphFn,
        text_cache::{Atlas, AtlasDimensions, GlyphCacheResult, TextCache},
    },
    temp_buffer::TempBuffer,
    text::text_trait,
};

const LOCALE: PCWSTR = w!("en-us");
const ATLAS_PADDING: f32 = 2.0;

#[derive(Debug, Clone, Copy)]
pub struct Glyph<'a> {
    pub index: u16,
    run: &'a DWRITE_GLYPH_RUN,
    measuring_mode: DWRITE_MEASURING_MODE,
}

struct DrawingContext<'a> {
    text: &'a mut Text,
    text_cache: &'a mut TextCache,
    glyph_cache_result: GlyphCacheResult,
    glyph_fn: GlyphFn,
}

pub struct Text {
    dwrite_factory: IDWriteFactory4,
    d2d_factory: ID2D1Factory4,
    imaging_factory: IWICImagingFactory,
    text_renderer: Option<IDWriteTextRenderer>,

    text_format: IDWriteTextFormat,
    text_rendering_params: IDWriteRenderingParams3,

    wide_characters: TempBuffer<u16>,

    glyph_metrics_scale: f32,
    glyph_width: f32,
    line_height: f32,
}

impl Text {
    pub unsafe fn new(font_name: &str, font_size: f32, scale: f32) -> Result<Self> {
        let font_size = (scale * font_size).floor();
        let font_name = HSTRING::from(font_name);

        let debug_level = if cfg!(debug_assertions) {
            D2D1_DEBUG_LEVEL_INFORMATION
        } else {
            D2D1_DEBUG_LEVEL_NONE
        };

        let d2d_factory: ID2D1Factory4 = D2D1CreateFactory(
            D2D1_FACTORY_TYPE_SINGLE_THREADED,
            Some(&D2D1_FACTORY_OPTIONS {
                debugLevel: debug_level,
            }),
        )?;

        let dwrite_factory: IDWriteFactory4 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

        let text_format = dwrite_factory.CreateTextFormat(
            &font_name,
            None,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            font_size,
            LOCALE,
        )?;

        let font_collection = text_format.GetFontCollection()?;

        let mut font_index = 0u32;
        let mut font_exists = FALSE;
        font_collection.FindFamilyName(&font_name, &mut font_index, &mut font_exists)?;

        if !font_exists.as_bool() {
            return Err(Error::new(E_FAIL, "Font not found"));
        }

        let font_family = font_collection.GetFontFamily(font_index)?;

        let font = font_family
            .GetFirstMatchingFont(
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
            )?
            .cast::<IDWriteFont1>()?;

        let font_face = font.CreateFontFace()?;

        let mut font_metrics = DWRITE_FONT_METRICS::default();
        font_face.GetMetrics(&mut font_metrics);

        let glyph_metrics_scale = font_size / font_metrics.designUnitsPerEm as f32;

        let mut m_glyph_index = 0u16;
        font_face.GetGlyphIndices(['M' as u32].as_ptr(), 1, &mut m_glyph_index)?;

        let mut m_glyph_metrics = DWRITE_GLYPH_METRICS::default();
        font_face.GetDesignGlyphMetrics(&m_glyph_index, 1, &mut m_glyph_metrics, false)?;

        let glyph_width = ((m_glyph_metrics.advanceWidth as f32) * glyph_metrics_scale).ceil();
        let line_height = ((font_metrics.ascent as f32
            + font_metrics.descent as f32
            + font_metrics.lineGap as f32)
            * glyph_metrics_scale)
            .ceil();

        let imaging_factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;

        let text_rendering_params = dwrite_factory.CreateCustomRenderingParams(
            1.0,
            1.0,
            1.0,
            1.0,
            DWRITE_PIXEL_GEOMETRY_RGB,
            DWRITE_RENDERING_MODE1_DEFAULT,
            DWRITE_GRID_FIT_MODE_DEFAULT,
        )?;

        Ok(Self {
            dwrite_factory,
            d2d_factory,
            imaging_factory,
            text_renderer: Some(TextRenderer {}.into()),

            text_format,
            text_rendering_params,

            wide_characters: TempBuffer::new(),

            glyph_metrics_scale,
            glyph_width,
            line_height,
        })
    }

    pub unsafe fn generate_atlas(&mut self, glyph: Glyph) -> Result<Atlas> {
        let font_face = glyph.run.fontFace.as_ref().unwrap();

        let mut font_metrics = DWRITE_FONT_METRICS::default();
        font_face.GetMetrics(&mut font_metrics);

        let mut glyph_metrics = DWRITE_GLYPH_METRICS::default();
        font_face.GetDesignGlyphMetrics(&glyph.index, 1, &mut glyph_metrics, false)?;

        let left = glyph_metrics.leftSideBearing as f32 * self.glyph_metrics_scale;
        let top = (glyph_metrics.topSideBearing - glyph_metrics.verticalOriginY) as f32
            * self.glyph_metrics_scale;
        let right = (glyph_metrics.advanceWidth as f32 - glyph_metrics.rightSideBearing as f32)
            * self.glyph_metrics_scale;
        let bottom = (glyph_metrics.advanceHeight as f32
            - glyph_metrics.bottomSideBearing as f32
            - glyph_metrics.verticalOriginY as f32)
            * self.glyph_metrics_scale;

        let width = (right.ceil() - left.ceil() + ATLAS_PADDING) as u32;
        let height = (bottom.ceil() - top.ceil() + ATLAS_PADDING) as u32;

        let origin = Vector2 {
            X: -left.ceil() + ATLAS_PADDING,
            Y: -top.ceil() + ATLAS_PADDING,
        };

        let translated_runs = self.dwrite_factory.TranslateColorGlyphRun(
            origin,
            glyph.run,
            None,
            DWRITE_GLYPH_IMAGE_FORMATS_PNG
                | DWRITE_GLYPH_IMAGE_FORMATS_SVG
                | DWRITE_GLYPH_IMAGE_FORMATS_COLR,
            DWRITE_MEASURING_MODE_NATURAL,
            None,
            0,
        );

        let has_color_glyphs = translated_runs.is_ok();

        let (format, alpha_mode) = if has_color_glyphs {
            (GUID_WICPixelFormat32bppPBGRA, D2D1_ALPHA_MODE_PREMULTIPLIED)
        } else {
            (GUID_WICPixelFormat32bppBGR, D2D1_ALPHA_MODE_IGNORE)
        };

        let bitmap =
            self.imaging_factory
                .CreateBitmap(width, height, &format, WICBitmapCacheOnDemand)?;

        let render_target: ID2D1RenderTarget = self.d2d_factory.CreateWicBitmapRenderTarget(
            &bitmap,
            &D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_UNKNOWN,
                    alphaMode: alpha_mode,
                },
                dpiX: 0.0,
                dpiY: 0.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            },
        )?;

        let context: ID2D1DeviceContext4 = render_target.cast()?;

        context.SetTextRenderingParams(&self.text_rendering_params);

        let brush = context.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            None,
        )?;

        context.BeginDraw();

        context.Clear(Some(&D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }));

        match translated_runs {
            Ok(runs) => loop {
                let Ok(TRUE) = runs.MoveNext() else {
                    break;
                };

                let run = unsafe { runs.GetCurrentRun()?.as_mut() }.unwrap();

                match run.glyphImageFormat {
                    DWRITE_GLYPH_IMAGE_FORMATS_PNG => {
                        context.DrawColorBitmapGlyphRun(
                            run.glyphImageFormat,
                            origin,
                            &run.Base.glyphRun,
                            glyph.measuring_mode,
                            D2D1_COLOR_BITMAP_GLYPH_SNAP_OPTION_DEFAULT,
                        );
                    }
                    DWRITE_GLYPH_IMAGE_FORMATS_SVG => {
                        context.DrawSvgGlyphRun(
                            origin,
                            &run.Base.glyphRun,
                            &brush,
                            None,
                            0,
                            glyph.measuring_mode,
                        );
                    }
                    DWRITE_GLYPH_IMAGE_FORMATS_COLR => {
                        if run.Base.paletteIndex != 0xFFFF {
                            brush.SetColor(&D2D1_COLOR_F {
                                r: run.Base.runColor.r,
                                g: run.Base.runColor.g,
                                b: run.Base.runColor.b,
                                a: run.Base.runColor.a,
                            });
                        } else {
                            brush.SetColor(&D2D1_COLOR_F {
                                r: 1.0,
                                g: 1.0,
                                b: 1.0,
                                a: 1.0,
                            });
                        }

                        context.DrawGlyphRun(
                            origin,
                            &run.Base.glyphRun,
                            Some(run.Base.glyphRunDescription),
                            &brush,
                            glyph.measuring_mode,
                        );
                    }
                    _ => unreachable!(),
                }
            },
            Err(err) => {
                assert!(err.code() == DWRITE_E_NOCOLOR);

                context.DrawGlyphRun(origin, glyph.run, None, &brush, glyph.measuring_mode);
            }
        }

        context.EndDraw(None, None)?;

        let mut raw_data = vec![0u8; (width * height * 4) as usize];
        bitmap.CopyPixels(null(), width * 4, &mut raw_data)?;

        for i in (0..raw_data.len()).step_by(4) {
            let b = raw_data[i];
            let g = raw_data[i + 1];
            let r = raw_data[i + 2];

            raw_data[i] = r;
            raw_data[i + 1] = g;
            raw_data[i + 2] = b;
        }

        if !has_color_glyphs {
            // Prevent texture bleeding if a color glyph is added to the atlas after these ones.
            // Color glyphs care about alpha, even if these glyphs don't.
            for i in (0..raw_data.len()).step_by(4) {
                raw_data[i + 3] = 0;
            }
        }

        Ok(Atlas {
            data: raw_data,
            dimensions: AtlasDimensions {
                origin_x: left.ceil(),
                origin_y: bottom.ceil(),
                width: width as usize,
                height: height as usize,
                glyph_width: self.glyph_width as usize,
                glyph_height: height as usize,
                line_height: self.line_height as usize,
            },
            has_color_glyphs,
        })
    }

    pub unsafe fn get_glyphs(
        &mut self,
        text_cache: &mut TextCache,
        glyph_cache_result: GlyphCacheResult,
        text: text_trait!(),
        glyph_fn: GlyphFn,
    ) -> GlyphCacheResult {
        let wide_characters = self.wide_characters.get_mut();

        for c in text {
            let c = *c.borrow();
            let mut dst = [0u16; 2];

            for wide_c in c.encode_utf16(&mut dst) {
                wide_characters.push(*wide_c);
            }
        }

        let text_layout = self
            .dwrite_factory
            .CreateTextLayout(wide_characters, &self.text_format, f32::MAX, f32::MAX)
            .unwrap();

        let text_renderer = self.text_renderer.take().unwrap();

        let drawing_context = DrawingContext {
            text: self,
            text_cache,
            glyph_cache_result,
            glyph_fn,
        };

        text_layout
            .Draw(
                Some(&drawing_context as *const _ as _),
                &text_renderer,
                0.0,
                0.0,
            )
            .unwrap();

        let glyph_cache_result = drawing_context.glyph_cache_result;

        self.text_renderer = Some(text_renderer);

        glyph_cache_result
    }
}

#[implement(IDWriteTextRenderer)]
struct TextRenderer {}

impl IDWriteTextRenderer_Impl for TextRenderer_Impl {
    fn DrawGlyphRun(
        &self,
        client_drawing_context: *const c_void,
        _baseline_origin_x: f32,
        _baseline_origin_y: f32,
        measuring_mode: DWRITE_MEASURING_MODE,
        glyph_run: *const DWRITE_GLYPH_RUN,
        _glyph_run_description: *const DWRITE_GLYPH_RUN_DESCRIPTION,
        _client_drawing_effect: Ref<IUnknown>,
    ) -> Result<()> {
        let context = client_drawing_context as *mut DrawingContext;
        let context = unsafe { context.as_mut().unwrap() };

        let glyph_run = unsafe { glyph_run.as_ref() }.unwrap();
        let glyph_indices =
            unsafe { from_raw_parts(glyph_run.glyphIndices, glyph_run.glyphCount as usize) };

        for glyph_index in glyph_indices {
            let glyph_run = DWRITE_GLYPH_RUN {
                fontFace: glyph_run.fontFace.clone(),
                fontEmSize: glyph_run.fontEmSize,
                glyphCount: 1,
                glyphIndices: glyph_index,
                glyphAdvances: [0.0].as_ptr(),
                glyphOffsets: [DWRITE_GLYPH_OFFSET::default()].as_ptr(),
                isSideways: FALSE,
                bidiLevel: 0,
            };

            let glyph = Glyph {
                index: *glyph_index,
                run: &glyph_run,
                measuring_mode,
            };

            context.glyph_cache_result = (context.glyph_fn)(
                context.text,
                context.text_cache,
                glyph,
                context.glyph_cache_result,
            );
        }

        Ok(())
    }

    fn DrawUnderline(
        &self,
        _client_drawing_context: *const c_void,
        _baseline_origin_x: f32,
        _baseline_origin_y: f32,
        _underline: *const DWRITE_UNDERLINE,
        _client_drawing_effect: Ref<IUnknown>,
    ) -> Result<()> {
        Ok(())
    }

    fn DrawStrikethrough(
        &self,
        _client_drawing_context: *const c_void,
        _baseline_origin_x: f32,
        _baseline_origin_y: f32,
        _strikethrough: *const DWRITE_STRIKETHROUGH,
        _client_drawing_effect: Ref<IUnknown>,
    ) -> Result<()> {
        Ok(())
    }

    fn DrawInlineObject(
        &self,
        _client_drawing_context: *const c_void,
        _origin_x: f32,
        _origin_y: f32,
        _inline_object: Ref<IDWriteInlineObject>,
        _is_sideways: BOOL,
        _is_right_to_left: BOOL,
        _client_drawing_effect: Ref<IUnknown>,
    ) -> Result<()> {
        Ok(())
    }
}

impl IDWritePixelSnapping_Impl for TextRenderer_Impl {
    fn IsPixelSnappingDisabled(&self, _client_drawing_context: *const c_void) -> Result<BOOL> {
        Ok(FALSE)
    }

    fn GetCurrentTransform(
        &self,
        _client_drawing_context: *const c_void,
        transform: *mut DWRITE_MATRIX,
    ) -> Result<()> {
        let transform = transform as *mut Matrix3x2;

        unsafe {
            *transform = Matrix3x2::identity();
        }

        Ok(())
    }

    fn GetPixelsPerDip(&self, _client_drawing_context: *const c_void) -> Result<f32> {
        Ok(1.0)
    }
}
