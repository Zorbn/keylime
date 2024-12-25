use std::{mem::ManuallyDrop, ops::RangeInclusive};

use windows::{
    core::{Error, Result, HSTRING},
    Win32::{
        Foundation::{E_FAIL, FALSE},
        Graphics::DirectWrite::*,
    },
};

use crate::platform::text::{Atlas, AtlasDimensions};

pub struct Text {
    dwrite_factory: IDWriteFactory,

    font_face: IDWriteFontFace,
    font_size: f32,

    glyph_width: f32,
    line_height: f32,
}

impl Text {
    pub unsafe fn new(font_name: &str, font_size: f32, scale: f32) -> Result<Self> {
        let font_size = (scale * font_size).floor();
        let font_name = HSTRING::from(font_name);

        let dwrite_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

        let mut font_collection_result = None;
        dwrite_factory.GetSystemFontCollection(&mut font_collection_result, FALSE)?;
        let font_collection = font_collection_result.unwrap();

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

        Ok(Self {
            dwrite_factory,

            font_face,
            font_size,

            glyph_width,
            line_height,
        })
    }

    pub unsafe fn generate_atlas(&mut self, characters: RangeInclusive<char>) -> Result<Atlas> {
        let atlas_size = *characters.end() as usize - *characters.start() as usize + 1;

        let glyph_step_x = self.glyph_width.ceil() + 1.0;
        let glyph_offsets = vec![DWRITE_GLYPH_OFFSET::default(); atlas_size];

        let mut glyph_indices = vec![0u16; atlas_size];
        let mut glyph_advances = vec![0.0; atlas_size];

        for (i, c) in characters.enumerate() {
            let code_points = [c as u32];

            self.font_face.GetGlyphIndices(
                code_points.as_ptr(),
                code_points.len() as u32,
                glyph_indices.as_mut_ptr().add(i),
            )?;

            glyph_advances[i] = glyph_step_x;
        }

        let rendering_params = self.dwrite_factory.CreateRenderingParams()?;

        let rendering_mode = self.font_face.GetRecommendedRenderingMode(
            self.font_size,
            1.0,
            DWRITE_MEASURING_MODE_NATURAL,
            &rendering_params,
        )?;

        let glyph_run_analysis = self.dwrite_factory.CreateGlyphRunAnalysis(
            &DWRITE_GLYPH_RUN {
                fontFace: ManuallyDrop::new(Some(self.font_face.clone())),
                fontEmSize: self.font_size,
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
        )?;

        let desired_bounds =
            glyph_run_analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_CLEARTYPE_3x1)?;

        let unshifted_atlas_width = (desired_bounds.right - desired_bounds.left) as usize;
        let atlas_width = (desired_bounds.right + desired_bounds.left.max(0)) as usize;
        let atlas_height = (desired_bounds.bottom - desired_bounds.top) as usize;

        let mut unshifted_result = vec![0u8; unshifted_atlas_width * atlas_height * 3];
        let mut result = vec![0u8; atlas_width * atlas_height * 4];

        glyph_run_analysis.CreateAlphaTexture(
            DWRITE_TEXTURE_CLEARTYPE_3x1,
            &desired_bounds,
            &mut unshifted_result,
        )?;

        let src_shift_width = desired_bounds.left.min(0).abs() as usize;
        let dst_shift_width = desired_bounds.left.max(0) as usize;

        for y in 0..atlas_height {
            for x in 0..atlas_width.min(unshifted_atlas_width) {
                let src_i = (src_shift_width + x + y * unshifted_atlas_width) * 3;
                let dst_i = (dst_shift_width + x + y * atlas_width) * 4;

                result[dst_i] = unshifted_result[src_i];
                result[dst_i + 1] = unshifted_result[src_i + 1];
                result[dst_i + 2] = unshifted_result[src_i + 2];
                result[dst_i + 3] = 0;
            }
        }

        Ok(Atlas {
            data: result,
            dimensions: AtlasDimensions {
                width: atlas_width,
                height: atlas_height,
                glyph_step_x: glyph_step_x as usize,
                glyph_width: self.glyph_width.floor() as usize,
                glyph_height: (desired_bounds.bottom - desired_bounds.top) as usize,
                line_height: self.line_height.floor() as usize,
            },
            has_color_glyphs: false,
        })
    }
}
