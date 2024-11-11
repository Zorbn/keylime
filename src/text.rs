use std::mem::ManuallyDrop;

use windows::{
    core::{w, Result},
    Win32::{Foundation::FALSE, Graphics::DirectWrite::*},
};

const FONT_SIZE: f32 = 13.0;

pub struct Text {
    dwrite_factory: IDWriteFactory,
    text_format: IDWriteTextFormat,

    font_collection: IDWriteFontCollection,
    font_family: IDWriteFontFamily,
    font: IDWriteFont,

    glyph_width: f32,
    glyph_height: f32,
}

impl Text {
    pub unsafe fn new() -> Result<Self> {
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

        let mut font_collection_result = None;

        dwrite_factory.GetSystemFontCollection(&mut font_collection_result, FALSE)?;

        let font_collection = font_collection_result.unwrap();

        let mut font_index = 0u32;
        let mut font_exists = FALSE;
        font_collection.FindFamilyName(w!("Consolas"), &mut font_index, &mut font_exists)?;

        assert!(font_exists.as_bool());

        let font_family = font_collection.GetFontFamily(font_index)?;

        let font = font_family.GetFirstMatchingFont(
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STRETCH_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
        )?;

        let glyph_layout =
            dwrite_factory.CreateTextLayout(w!("M").as_wide(), &text_format, 1000.0, 1000.0)?;

        // TODO: Get metrics from font instead.
        let mut glyph_metrics = DWRITE_TEXT_METRICS::default();
        glyph_layout.GetMetrics(&mut glyph_metrics)?;

        let glyph_width = glyph_metrics.width;
        let glyph_height = glyph_metrics.height;

        Ok(Self {
            dwrite_factory,
            text_format,

            font_collection,
            font_family,
            font,

            glyph_width,
            glyph_height,
        })
    }

    pub unsafe fn generate_atlas(&mut self) -> Result<(Vec<u8>, usize, usize)> {
        let font_face = self.font.CreateFontFace().unwrap();

        let glyph_offsets = [DWRITE_GLYPH_OFFSET::default(); 5];
        let mut glyph_indices = [0u16; 5];
        let mut glyph_advances = [0.0; 5];

        for (i, c) in "world".chars().enumerate() {
            let code_points = [c as u32];

            font_face.GetGlyphIndices(
                code_points.as_ptr(),
                code_points.len() as u32,
                glyph_indices.as_mut_ptr().add(i),
            )?;

            glyph_advances[i] = self.glyph_width;
        }

        let rendering_params = self.dwrite_factory.CreateRenderingParams().unwrap();

        let rendering_mode = font_face
            .GetRecommendedRenderingMode(
                FONT_SIZE,
                1.0,
                DWRITE_MEASURING_MODE_NATURAL,
                &rendering_params,
            )
            .unwrap();

        let glyph_run_analysis = self
            .dwrite_factory
            .CreateGlyphRunAnalysis(
                &DWRITE_GLYPH_RUN {
                    fontFace: ManuallyDrop::new(Some(font_face)),
                    fontEmSize: FONT_SIZE,
                    glyphCount: glyph_indices.len() as u32,
                    glyphIndices: glyph_indices.as_ptr(),
                    glyphAdvances: glyph_advances.as_ptr(),
                    glyphOffsets: glyph_offsets.as_ptr(),
                    isSideways: FALSE,
                    bidiLevel: 0,
                },
                1.0,
                None,
                rendering_mode,
                DWRITE_MEASURING_MODE_NATURAL,
                0.0,
                0.0,
            )
            .unwrap();

        let desired_bounds =
            glyph_run_analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_CLEARTYPE_3x1)?;
        println!("{:?}", desired_bounds);

        let atlas_width = (desired_bounds.right - desired_bounds.left) as usize;
        let atlas_height = (desired_bounds.bottom - desired_bounds.top) as usize;

        let mut result = vec![0u8; atlas_width * atlas_height * 3];

        glyph_run_analysis
            .CreateAlphaTexture(DWRITE_TEXTURE_CLEARTYPE_3x1, &desired_bounds, &mut result)
            .unwrap();

        // TODO: This can maybe be done in place starting from the end?
        let mut rgba_result = vec![0u8; atlas_width * atlas_height * 4];

        for i in 0..(atlas_width * atlas_height) {
            let source_index = i * 3;
            let destination_index = i * 4;

            rgba_result[destination_index] = result[source_index];
            rgba_result[destination_index + 1] = result[source_index + 1];
            rgba_result[destination_index + 2] = result[source_index + 2];
            rgba_result[destination_index + 3] =
                result[source_index].max(result[source_index + 1].max(result[source_index + 2]));
        }

        Ok((rgba_result, atlas_width, atlas_height))
    }
}
