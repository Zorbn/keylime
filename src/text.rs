use std::mem::ManuallyDrop;

use windows::{
    core::{w, Interface, Result, PCWSTR},
    Win32::{Foundation::FALSE, Graphics::DirectWrite::*},
};

const FONT_NAME: PCWSTR = w!("Consolas");
const FONT_SIZE: f32 = 13.0;

pub struct Atlas {
    pub data: Vec<u8>,
    pub dimensions: AtlasDimensions,
}

#[derive(Debug)]
pub struct AtlasDimensions {
    pub width: usize,
    pub height: usize,
    pub glyph_offset_x: f32,
    pub glyph_step_x: f32,
    pub glyph_width: f32,
    pub glyph_height: f32,
}

pub struct Text {
    dwrite_factory: IDWriteFactory,

    font_collection: IDWriteFontCollection,
    font_family: IDWriteFontFamily,
    font: IDWriteFont,
    font_face: IDWriteFontFace,

    glyph_width: f32,
    glyph_height: f32,
    glyph_metrics_scale: f32,
}

impl Text {
    pub unsafe fn new() -> Result<Self> {
        let dwrite_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

        let mut font_collection_result = None;
        dwrite_factory.GetSystemFontCollection(&mut font_collection_result, FALSE)?;
        let font_collection = font_collection_result.unwrap();

        let mut font_index = 0u32;
        let mut font_exists = FALSE;
        font_collection.FindFamilyName(FONT_NAME, &mut font_index, &mut font_exists)?;

        assert!(font_exists.as_bool());

        let font_family = font_collection.GetFontFamily(font_index)?;

        let font = font_family.GetFirstMatchingFont(
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STRETCH_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
        )?;

        let font_face = font.CreateFontFace().unwrap();

        let mut font_metrics = DWRITE_FONT_METRICS::default();
        font_face.GetMetrics(&mut font_metrics);

        let glyph_metrics_scale = FONT_SIZE / font_metrics.designUnitsPerEm as f32;

        let mut m_glyph_index = 0u16;
        font_face.GetGlyphIndices(['M' as u32].as_ptr(), 1, &mut m_glyph_index)?;

        let mut m_glyph_metrics = DWRITE_GLYPH_METRICS::default();
        font_face.GetDesignGlyphMetrics(&m_glyph_index, 1, &mut m_glyph_metrics, FALSE)?;

        let glyph_width = (m_glyph_metrics.advanceWidth as f32) * glyph_metrics_scale;
        let glyph_height = (font_metrics.ascent as f32
            + font_metrics.descent as f32
            + font_metrics.lineGap as f32
            - m_glyph_metrics.topSideBearing as f32
            - m_glyph_metrics.bottomSideBearing as f32)
            * glyph_metrics_scale;

        println!("{glyph_width} {glyph_height}");
        println!(
            "is monospaced: {}",
            font_face
                .cast::<IDWriteFontFace1>()
                .unwrap()
                .IsMonospacedFont()
                .as_bool()
        );

        Ok(Self {
            dwrite_factory,

            font_collection,
            font_family,
            font,
            font_face,

            glyph_width,
            glyph_height,
            glyph_metrics_scale,
        })
    }

    // For now the atlas is static and only supports ASCII characters.
    // It could be upgraded to support any character and use the atlas
    // as a cache that gets updated when new characters are needed.
    pub unsafe fn generate_atlas(&mut self) -> Result<Atlas> {
        const ATLAS_SIZE: usize = (b'~' - b' ') as usize;

        let glyph_offsets = [DWRITE_GLYPH_OFFSET::default(); ATLAS_SIZE];
        let mut glyph_indices = [0u16; ATLAS_SIZE];
        let mut glyph_advances = [0.0; ATLAS_SIZE];

        for i in 0..ATLAS_SIZE {
            let code_points = [b' ' as u32 + i as u32 + 1];

            let mut glyph_index = 0u16;
            self.font_face
                .GetGlyphIndices(code_points.as_ptr(), 1, &mut glyph_index)?;

            let mut glyph_metrics = DWRITE_GLYPH_METRICS::default();
            self.font_face
                .GetDesignGlyphMetrics(&glyph_index, 1, &mut glyph_metrics, FALSE)?;

            assert!(
                (glyph_metrics.advanceWidth as f32 * self.glyph_metrics_scale) == self.glyph_width
            );

            self.font_face.GetGlyphIndices(
                code_points.as_ptr(),
                code_points.len() as u32,
                glyph_indices.as_mut_ptr().add(i),
            )?;

            glyph_advances[i] = self.glyph_width.ceil();
        }

        let rendering_params = self.dwrite_factory.CreateRenderingParams().unwrap();

        let rendering_mode = self
            .font_face
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
                    fontFace: ManuallyDrop::new(Some(self.font_face.clone())),
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

        let mut result = vec![0u8; atlas_width * atlas_height * 4];

        glyph_run_analysis
            .CreateAlphaTexture(DWRITE_TEXTURE_CLEARTYPE_3x1, &desired_bounds, &mut result)
            .unwrap();

        for i in (0..(atlas_width * atlas_height)).rev() {
            let source_index = i * 3;
            let destination_index = i * 4;

            result[destination_index] = result[source_index];
            result[destination_index + 1] = result[source_index + 1];
            result[destination_index + 2] = result[source_index + 2];
        }

        println!("atlas size: {}, {}", atlas_width, atlas_height);

        Ok(Atlas {
            data: result,
            dimensions: AtlasDimensions {
                width: atlas_width,
                height: atlas_height,
                glyph_offset_x: desired_bounds.left as f32,
                glyph_step_x: self.glyph_width.ceil(),
                glyph_width: self.glyph_width,
                glyph_height: (desired_bounds.bottom - desired_bounds.top) as f32,
            },
        })
    }
}
