use std::{mem::ManuallyDrop, ops::RangeInclusive, ptr::null};

use windows::{
    core::{implement, w, Error, Interface, Result, HSTRING, PCWSTR},
    Win32::{
        Foundation::{DWRITE_E_NOCOLOR, E_FAIL, FALSE},
        Graphics::{
            Direct2D::{
                Common::{D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_POINT_2F},
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
use Common::{D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED};

use crate::platform::text::{Atlas, AtlasDimensions};

const LOCALE: PCWSTR = w!("en-us");
const ATLAS_PADDING: f32 = 2.0;

pub struct Text {
    dwrite_factory: IDWriteFactory4,
    d2d_factory: ID2D1Factory,
    imaging_factory: IWICImagingFactory,

    font_size: f32,

    text_format: IDWriteTextFormat,
    text_rendering_params: IDWriteRenderingParams3,
    typography: IDWriteTypography,
    system_font_fallback: IDWriteFontFallback,
    system_font_collection: IDWriteFontCollection1,

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

        let font = font_family.GetFirstMatchingFont(
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STRETCH_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
        )?;

        let font_face = font.CreateFontFace()?;

        let mut font_metrics = DWRITE_FONT_METRICS::default();
        font_face.GetMetrics(&mut font_metrics);

        let glyph_metrics_scale = font_size / font_metrics.designUnitsPerEm as f32;

        let mut m_glyph_index = 0u16;
        font_face.GetGlyphIndices(['M' as u32].as_ptr(), 1, &mut m_glyph_index)?;

        let mut m_glyph_metrics = DWRITE_GLYPH_METRICS::default();
        font_face.GetDesignGlyphMetrics(&m_glyph_index, 1, &mut m_glyph_metrics, FALSE)?;

        let glyph_width = (m_glyph_metrics.advanceWidth as f32) * glyph_metrics_scale;
        let line_height = (font_metrics.ascent as f32
            + font_metrics.descent as f32
            + font_metrics.lineGap as f32)
            * glyph_metrics_scale;

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

        let typography = dwrite_factory.CreateTypography()?;

        let system_font_fallback = dwrite_factory.GetSystemFontFallback()?;
        let mut system_font_collection = None;
        dwrite_factory.GetSystemFontCollection(FALSE, &mut system_font_collection, FALSE)?;
        let system_font_collection = system_font_collection.unwrap();

        Ok(Self {
            dwrite_factory,
            d2d_factory,
            imaging_factory,

            font_size,

            text_format,
            text_rendering_params,
            typography,
            system_font_fallback,
            system_font_collection,

            glyph_width,
            line_height,
        })
    }

    pub unsafe fn generate_atlas(&mut self, characters: RangeInclusive<char>) -> Result<Atlas> {
        let atlas_size = *characters.end() as usize - *characters.start() as usize + 1;

        let first_character = *characters.start();
        let mut wide_characters = Vec::new();

        for c in characters {
            let mut dst = [0u16; 2];

            for wide_c in c.encode_utf16(&mut dst) {
                wide_characters.push(*wide_c);
            }
        }

        let glyph_step_x = self.glyph_width.ceil() + ATLAS_PADDING;

        let text_layout = self.dwrite_factory.CreateTextLayout(
            &wide_characters,
            &self.text_format,
            f32::MAX,
            f32::MAX,
        )?;

        let has_color_glyphs = self
            .has_color_glyphs(first_character, &wide_characters)
            .unwrap_or(false);

        let range = DWRITE_TEXT_RANGE {
            startPosition: 0,
            length: atlas_size as u32,
        };

        let text_layout = text_layout.cast::<IDWriteTextLayout1>()?;
        text_layout.SetTypography(&self.typography, range)?;
        text_layout.SetCharacterSpacing(0.0, glyph_step_x - self.glyph_width, 0.0, range)?;

        let mut text_metrics = DWRITE_TEXT_METRICS::default();
        text_layout.GetMetrics(&mut text_metrics)?;

        let width = text_metrics.width.ceil() as u32;
        let height = text_metrics.height.ceil() as u32;

        let (format, alpha_mode) = if has_color_glyphs {
            (GUID_WICPixelFormat32bppPBGRA, D2D1_ALPHA_MODE_PREMULTIPLIED)
        } else {
            (GUID_WICPixelFormat32bppBGR, D2D1_ALPHA_MODE_IGNORE)
        };

        let bitmap =
            self.imaging_factory
                .CreateBitmap(width, height, &format, WICBitmapCacheOnDemand)?;

        let render_target = self.d2d_factory.CreateWicBitmapRenderTarget(
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

        render_target.SetTextRenderingParams(&self.text_rendering_params);

        let brush = render_target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            None,
        )?;

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
            D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
        );

        render_target.EndDraw(None, None)?;

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
                width: width as usize,
                height: height as usize,
                glyph_step_x: glyph_step_x as usize,
                glyph_width: self.glyph_width.floor() as usize,
                glyph_height: height as usize,
                line_height: self.line_height.floor() as usize,
            },
            has_color_glyphs,
        })
    }

    unsafe fn has_color_glyphs(&mut self, c: char, wide_characters: &[u16]) -> Result<bool> {
        let analysis_source = AnalysisSource {
            string: wide_characters,
        };

        let analysis_source: IDWriteTextAnalysisSource = analysis_source.into();

        let mut mapped_length = 0;
        let mut mapped_font = None;
        let mut scale = 0.0;

        self.system_font_fallback.MapCharacters(
            &analysis_source,
            0,
            wide_characters.len() as u32,
            &self.system_font_collection,
            None,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            &mut mapped_length,
            &mut mapped_font,
            &mut scale,
        )?;

        let mapped_font = mapped_font.ok_or(Error::new(E_FAIL, "Mapped font not found"))?;
        let mapped_font_face = mapped_font.CreateFontFace()?;

        let mut glyph_indices = [0u16];

        mapped_font_face.GetGlyphIndices([c as u32].as_ptr(), 1, glyph_indices.as_mut_ptr())?;

        let glyph_run = DWRITE_GLYPH_RUN {
            fontFace: ManuallyDrop::new(Some(mapped_font_face.clone())),
            fontEmSize: self.font_size,
            glyphCount: 1,
            glyphIndices: glyph_indices.as_ptr(),
            glyphAdvances: [0.0].as_ptr(),
            glyphOffsets: [DWRITE_GLYPH_OFFSET::default()].as_ptr(),
            isSideways: FALSE,
            bidiLevel: 0,
        };

        let result = self.dwrite_factory.TranslateColorGlyphRun(
            D2D_POINT_2F { x: 0.0, y: 0.0 },
            &glyph_run,
            None,
            DWRITE_GLYPH_IMAGE_FORMATS_PNG
                | DWRITE_GLYPH_IMAGE_FORMATS_SVG
                | DWRITE_GLYPH_IMAGE_FORMATS_COLR,
            DWRITE_MEASURING_MODE_NATURAL,
            None,
            0,
        );

        if let Err(err) = result {
            return Ok(err.code() != DWRITE_E_NOCOLOR);
        }

        Ok(true)
    }
}

#[implement(IDWriteTextAnalysisSource)]
struct AnalysisSource<'a> {
    string: &'a [u16],
}

impl<'a> IDWriteTextAnalysisSource_Impl for AnalysisSource_Impl<'a> {
    fn GetTextAtPosition(
        &self,
        _textposition: u32,
        textstring: *mut *mut u16,
        textlength: *mut u32,
    ) -> Result<()> {
        unsafe {
            *textstring = self.string.as_ptr() as *mut _;
            *textlength = self.string.len() as u32;
        }

        Ok(())
    }

    fn GetTextBeforePosition(
        &self,
        _textposition: u32,
        _textstring: *mut *mut u16,
        _textlength: *mut u32,
    ) -> Result<()> {
        Ok(())
    }

    fn GetParagraphReadingDirection(&self) -> DWRITE_READING_DIRECTION {
        DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
    }

    fn GetLocaleName(
        &self,
        _textposition: u32,
        textlength: *mut u32,
        localename: *mut *mut u16,
    ) -> Result<()> {
        unsafe {
            *textlength = self.string.len() as u32;
            *localename = LOCALE.as_ptr() as *mut _;
        }

        Ok(())
    }

    fn GetNumberSubstitution(
        &self,
        _textposition: u32,
        _textlength: *mut u32,
        _numbersubstitution: *mut Option<IDWriteNumberSubstitution>,
    ) -> Result<()> {
        Ok(())
    }
}
