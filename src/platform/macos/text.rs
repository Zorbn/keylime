use core::f64;
use std::{
    ops::Deref,
    ptr::{null_mut, NonNull},
};

use crate::platform::{
    text::GlyphFn,
    text_cache::{Atlas, AtlasDimensions, GlyphCacheResult, TextCache},
};

use super::result::Result;
use objc2_core_foundation::*;
use objc2_core_graphics::*;
use objc2_core_text::*;
use objc2_foundation::{NSMutableAttributedString, NSRange, NSString};

#[derive(Debug, Clone, Copy)]
pub struct Glyph<'a> {
    pub index: u16,
    has_color: bool,
    font: &'a CTFont,
}

pub struct Text {
    font: CFRetained<CTFont>,

    glyph_indices: Vec<u16>,

    glyph_width: usize,
    line_height: usize,
}

impl Text {
    pub unsafe fn new(font_name: &str, font_size: f32, scale: f32) -> Result<Self> {
        let font_name = CFString::from_str(font_name);
        let font_size = (font_size * scale).floor() as f64;

        let font = CTFontCreateWithNameAndOptions(
            &font_name,
            font_size,
            null_mut(),
            CTFontOptions::Default,
        );

        let loaded_font_name = CTFontCopyFamilyName(&font);

        let did_load_requested_font = CFStringCompare(
            &font_name,
            Some(&loaded_font_name),
            CFStringCompareFlags::empty(),
        ) == CFComparisonResult::CompareEqualTo;

        if !did_load_requested_font {
            return Err("Font not found");
        }

        let advance_glyphs = &[b'M' as u16];
        let mut advances = [CGSize::ZERO; 1];

        CTFontGetAdvancesForGlyphs(
            &font,
            CTFontOrientation::Horizontal,
            NonNull::from(&advance_glyphs[0]),
            &mut advances[0],
            1,
        );

        let line_height =
            CTFontGetAscent(&font) + CTFontGetDescent(&font) + CTFontGetLeading(&font);
        let line_height = line_height.ceil() as usize;

        let glyph_width = advances[0].width.floor() as usize;

        let attributes =
            CFDictionaryCreateMutable(kCFAllocatorDefault, 2, null_mut(), null_mut()).unwrap();

        CFDictionaryAddValue(
            Some(&attributes),
            kCTFontNameAttribute as *const _ as _,
            &*font_name as *const _ as _,
        );

        let descriptor = CTFontDescriptorCreateWithAttributes(&attributes);

        let font = CTFontCreateWithFontDescriptor(&descriptor, font_size, null_mut());

        Ok(Self {
            font,

            glyph_indices: Vec::new(),

            glyph_width,
            line_height,
        })
    }

    pub unsafe fn generate_atlas(&mut self, glyph: Glyph) -> Result<Atlas> {
        let mut glyphs = [glyph.index];
        let glyphs = NonNull::new(glyphs.as_mut_ptr()).unwrap();

        let rect = CTFontGetBoundingRectsForGlyphs(
            glyph.font,
            CTFontOrientation::Horizontal,
            glyphs,
            null_mut(),
            1,
        );

        let origin = rect.origin;
        let glyph_height = rect.size.height as usize;

        let rect = CGRect::new(
            CGPoint::ZERO,
            CGSize::new(rect.size.width.ceil() + 1.0, rect.size.height.ceil() + 1.0),
        );

        let mut raw_data = vec![0u8; rect.size.width as usize * rect.size.height as usize * 4];

        if rect.size.width == 0.0 || rect.size.height == 0.0 {
            return Ok(Atlas {
                data: raw_data,
                dimensions: AtlasDimensions::default(),
                has_color_glyphs: glyph.has_color,
            });
        }

        let bitmap_info =
            CGBitmapInfo(CGBitmapInfo::ByteOrder32Big.0 | CGImageAlphaInfo::PremultipliedLast.0);

        let rgb_color_space = CGColorSpaceCreateDeviceRGB();

        let context = CGBitmapContextCreateWithData(
            raw_data.as_mut_ptr() as _,
            rect.size.width as usize,
            rect.size.height as usize,
            8,
            (rect.size.width * 4.0) as usize,
            rgb_color_space.as_deref(),
            bitmap_info.0,
            CGBitmapContextReleaseDataCallback::None,
            null_mut(),
        )
        .unwrap();

        let mut positions = [CGPoint::new(-origin.x + 1.0, -origin.y + 1.0)];
        let positions = NonNull::new(positions.as_mut_ptr()).unwrap();

        CTFontDrawGlyphs(glyph.font, glyphs, positions, 1, &context);

        Ok(Atlas {
            data: raw_data,
            dimensions: AtlasDimensions {
                origin_x: origin.x.ceil() as f32,
                origin_y: -origin.y.ceil() as f32,
                width: rect.size.width as usize,
                height: rect.size.height as usize,
                glyph_width: self.glyph_width,
                glyph_height,
                line_height: self.line_height,
            },
            has_color_glyphs: glyph.has_color,
        })
    }

    pub unsafe fn get_glyphs(
        &mut self,
        text_cache: &mut TextCache,
        mut glyph_cache_result: GlyphCacheResult,
        text: &str,
        glyph_fn: GlyphFn,
    ) -> GlyphCacheResult {
        let attributed_string = NSMutableAttributedString::from_nsstring(&NSString::from_str(text));

        attributed_string.beginEditing();

        let font_attribute_name = (kCTFontAttributeName as *const _ as *const NSString)
            .as_ref()
            .unwrap();

        let attributed_string_len = attributed_string.length();
        attributed_string.addAttribute_value_range(
            font_attribute_name,
            self.font.as_ref(),
            NSRange::new(0, attributed_string_len),
        );

        attributed_string.endEditing();

        let line = CTLineCreateWithAttributedString(
            (attributed_string.deref() as *const _ as *const CFAttributedString)
                .as_ref()
                .unwrap(),
        );

        let runs = CTLineGetGlyphRuns(&line);
        let count = CFArrayGetCount(&runs);

        for i in 0..count {
            let run = (CFArrayGetValueAtIndex(&runs, i) as *const CTRun)
                .as_ref()
                .unwrap();

            let attributes = CTRunGetAttributes(run);
            let font = (CFDictionaryGetValue(&attributes, kCTFontAttributeName as *const _ as _)
                as *const CTFont)
                .as_ref()
                .unwrap();

            let traits = CTFontGetSymbolicTraits(font);
            let has_color = traits.contains(CTFontSymbolicTraits::ColorGlyphsTrait);

            let glyph_count = CTRunGetGlyphCount(run) as usize;

            self.glyph_indices.resize(glyph_count, 0);

            CTRunGetGlyphs(
                run,
                CFRange {
                    location: 0,
                    length: 0,
                },
                NonNull::new(self.glyph_indices.as_mut_ptr()).unwrap(),
            );

            for i in 0..glyph_count {
                let index = self.glyph_indices[i];

                glyph_cache_result = glyph_fn(
                    self,
                    text_cache,
                    Glyph {
                        index,
                        has_color,
                        font,
                    },
                    glyph_cache_result,
                );
            }
        }

        glyph_cache_result
    }
}
