use std::{ops::RangeInclusive, ptr::null};

use windows::{
    core::{w, Error, Interface, Result, HSTRING},
    Win32::{
        Foundation::{E_FAIL, FALSE},
        Graphics::{
            Direct2D::{
                Common::{D2D1_ALPHA_MODE_UNKNOWN, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_POINT_2F},
                *,
            },
            DirectWrite::*,
            Dxgi::Common::DXGI_FORMAT_UNKNOWN,
            Imaging::{
                CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
                WICBitmapCacheOnDemand,
            },
        },
        System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
    },
};

use crate::platform::text::{Atlas, AtlasDimensions};

// TODO: Replace unwraps with ?.
// TODO: Figure out why ClearType looks wrong.
// TODO: Support color glyphs: https://learn.microsoft.com/en-us/windows/win32/directwrite/color-fonts.

pub struct Text {
    dwrite_factory: IDWriteFactory1,
    d2d_factory: ID2D1Factory,
    imaging_factory: IWICImagingFactory,

    text_format: IDWriteTextFormat,
    typography: IDWriteTypography,

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

        let d2d_factory: ID2D1Factory = D2D1CreateFactory(
            D2D1_FACTORY_TYPE_SINGLE_THREADED,
            Some(&D2D1_FACTORY_OPTIONS {
                debugLevel: debug_level,
            }),
        )
        .unwrap();
        let dwrite_factory: IDWriteFactory1 =
            DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).unwrap();

        let text_format = dwrite_factory
            .CreateTextFormat(
                &font_name,
                None,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size,
                w!("en-us"),
            )
            .unwrap();

        let font_collection = text_format.GetFontCollection().unwrap();

        let mut font_index = 0u32;
        let mut font_exists = FALSE;
        font_collection
            .FindFamilyName(&font_name, &mut font_index, &mut font_exists)
            .unwrap();

        if !font_exists.as_bool() {
            return Err(Error::new(E_FAIL, "Font not found"));
        }

        let font_family = font_collection.GetFontFamily(font_index).unwrap();

        let font = font_family
            .GetFirstMatchingFont(
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
            )
            .unwrap();

        let font_face = font.CreateFontFace().unwrap();

        let mut font_metrics = DWRITE_FONT_METRICS::default();
        font_face.GetMetrics(&mut font_metrics);

        let glyph_metrics_scale = font_size / font_metrics.designUnitsPerEm as f32;

        let mut m_glyph_index = 0u16;
        font_face
            .GetGlyphIndices(['M' as u32].as_ptr(), 1, &mut m_glyph_index)
            .unwrap();

        let mut m_glyph_metrics = DWRITE_GLYPH_METRICS::default();
        font_face
            .GetDesignGlyphMetrics(&m_glyph_index, 1, &mut m_glyph_metrics, FALSE)
            .unwrap();

        let glyph_width = (m_glyph_metrics.advanceWidth as f32) * glyph_metrics_scale;
        let line_height = (font_metrics.ascent as f32
            + font_metrics.descent as f32
            + font_metrics.lineGap as f32)
            * glyph_metrics_scale;

        let imaging_factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).unwrap();

        let typography = dwrite_factory.CreateTypography()?;

        Ok(Self {
            dwrite_factory,
            d2d_factory,
            imaging_factory,

            text_format,
            typography,

            glyph_width,
            line_height,
        })
    }

    pub unsafe fn generate_atlas(&mut self, characters: RangeInclusive<char>) -> Result<Atlas> {
        let atlas_size = *characters.end() as usize - *characters.start() as usize + 1;

        let mut wide_characters = Vec::new();

        for c in characters {
            let mut dst = [0u16; 2];

            for wide_c in c.encode_utf16(&mut dst) {
                wide_characters.push(*wide_c);
            }
        }

        let glyph_step_x = self.glyph_width.ceil() + 1.0;

        let text_layout = self
            .dwrite_factory
            .CreateTextLayout(&wide_characters, &self.text_format, f32::MAX, f32::MAX)
            .unwrap();

        let range = DWRITE_TEXT_RANGE {
            startPosition: 0,
            length: atlas_size as u32,
        };

        let text_layout = text_layout.cast::<IDWriteTextLayout1>().unwrap();
        text_layout.SetTypography(&self.typography, range).unwrap();
        text_layout
            .SetCharacterSpacing(0.0, 0.0, glyph_step_x, range)
            .unwrap();

        let mut text_metrics = DWRITE_TEXT_METRICS::default();
        text_layout.GetMetrics(&mut text_metrics).unwrap();

        let width = text_metrics.width.ceil() as u32;
        let height = text_metrics.height.ceil() as u32;

        let bitmap = self
            .imaging_factory
            .CreateBitmap(
                width,
                height,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapCacheOnDemand,
            )
            .unwrap();

        let render_target = self
            .d2d_factory
            .CreateWicBitmapRenderTarget(
                &bitmap,
                &D2D1_RENDER_TARGET_PROPERTIES {
                    r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_UNKNOWN,
                        alphaMode: D2D1_ALPHA_MODE_UNKNOWN,
                    },
                    dpiX: 0.0,
                    dpiY: 0.0,
                    usage: D2D1_RENDER_TARGET_USAGE_GDI_COMPATIBLE,
                    minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
                },
            )
            .unwrap();

        render_target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);

        let brush = render_target
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

        render_target.DrawTextLayout(
            D2D_POINT_2F { x: 0.0, y: 0.0 },
            &text_layout,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
        );

        render_target.EndDraw(None, None).unwrap();

        let mut raw_data = vec![0u8; (width * height * 4) as usize];
        bitmap.CopyPixels(null(), width * 4, &mut raw_data).unwrap();

        for i in (0..raw_data.len()).step_by(4) {
            let b = raw_data[i];
            let g = raw_data[i + 1];
            let r = raw_data[i + 2];

            raw_data[i] = r;
            raw_data[i + 1] = g;
            raw_data[i + 2] = b;
        }

        Ok(Atlas {
            data: raw_data,
            dimensions: AtlasDimensions {
                width: width as usize,
                height: height as usize,
                glyph_step_x: glyph_step_x as usize,
                glyph_width: self.glyph_width.floor() as usize,
                glyph_height: height as usize,
                line_height: self.line_height.floor() as usize,
            },
            has_color_glyphs: false,
        })
    }
}
