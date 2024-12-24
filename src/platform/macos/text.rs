use core::f64;
use std::{
    ffi::c_void,
    ops::{Deref, RangeInclusive},
    ptr::{null_mut, NonNull},
};

use crate::platform::text::{Atlas, AtlasDimensions};

use super::result::Result;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_core_foundation::*;
use objc2_core_graphics::*;
use objc2_core_text::*;
use objc2_foundation::{NSMutableAttributedString, NSRange, NSRect, NSString};

pub struct Text {
    font: *mut AnyObject,

    glyph_width: usize,
    glyph_step_x: usize,
    line_height: usize,
}

impl Text {
    pub unsafe fn new(font_name: &str, font_size: f32, scale: f32) -> Result<Self> {
        let font_name = NSString::from_str(font_name);
        let font_size = (font_size * scale).floor() as f64;

        let font_name_cfstr = &*font_name as *const _ as CFStringRef;

        let font = CTFontCreateWithNameAndOptions(
            font_name_cfstr,
            font_size,
            null_mut(),
            CTFontOptions::kCTFontOptionsDefault,
        ) as *mut AnyObject;

        let loaded_font_name = CTFontCopyFamilyName(font as _);

        let did_load_requested_font = CFStringCompare(
            font_name_cfstr,
            loaded_font_name,
            CFStringCompareFlags::empty(),
        ) == CFComparisonResult::kCFCompareEqualTo;

        if !did_load_requested_font {
            return Err("Font not found");
        }

        let advance_glyphs = &[b'M' as u16];
        let mut advances = [CGSize::ZERO; 1];

        CTFontGetAdvancesForGlyphs(
            font as *mut c_void,
            CTFontOrientation::kCTFontOrientationHorizontal,
            NonNull::from(&advance_glyphs[0]),
            &mut advances[0],
            1,
        );

        let line_height =
            CTFontGetAscent(font as _) + CTFontGetDescent(font as _) + CTFontGetLeading(font as _);
        let line_height = line_height.ceil() as usize;

        let glyph_width = advances[0].width.floor() as usize;
        let glyph_step_x = advances[0].width.ceil() as usize + 1;

        let attributes = CFDictionaryCreateMutable(kCFAllocatorDefault, 2, null_mut(), null_mut());

        CFDictionaryAddValue(attributes, kCTFontNameAttribute, font_name_cfstr);

        let mut advance = glyph_step_x as f64;
        let advance = CFNumberCreate(
            kCFAllocatorDefault,
            CFNumberType::kCFNumberFloat64Type,
            &mut advance as *mut _ as _,
        );

        CFDictionaryAddValue(attributes, kCTFontFixedAdvanceAttribute, advance);

        let descriptor = CTFontDescriptorCreateWithAttributes(attributes);

        let font =
            CTFontCreateWithFontDescriptor(descriptor, font_size, null_mut()) as *mut AnyObject;

        Ok(Self {
            font,
            glyph_width,
            glyph_step_x,
            line_height,
        })
    }

    pub unsafe fn generate_atlas(&mut self, characters: RangeInclusive<char>) -> Result<Atlas> {
        let atlas_size = *characters.end() as usize - *characters.start() as usize + 1;
        let mut atlas_text = String::with_capacity(atlas_size);

        for c in characters {
            atlas_text.push(c);
        }

        let attributed_string =
            NSMutableAttributedString::from_nsstring(&NSString::from_str(&atlas_text));

        attributed_string.beginEditing();

        let font_attribute_name = kCTFontAttributeName as *const NSString;

        let attributed_string_len = attributed_string.length();
        attributed_string.addAttribute_value_range(
            &*font_attribute_name,
            &*self.font,
            NSRange::new(0, attributed_string_len),
        );

        attributed_string.endEditing();

        let (raw_data, rect) = Self::frameset(&attributed_string);
        let has_color_glyphs = Self::has_color_glyphs(&attributed_string);

        Ok(Atlas {
            data: raw_data,
            dimensions: AtlasDimensions {
                width: rect.size.width as usize,
                height: rect.size.height as usize,
                glyph_step_x: self.glyph_step_x,
                glyph_width: self.glyph_width,
                glyph_height: rect.size.height as usize,
                line_height: self.line_height,
            },
            has_color_glyphs,
        })
    }

    unsafe fn frameset(
        attributed_string: &Retained<NSMutableAttributedString>,
    ) -> (Vec<u8>, NSRect) {
        let framesetter =
            CTFramesetterCreateWithAttributedString(attributed_string.deref() as *const _ as _);

        let size = CTFramesetterSuggestFrameSizeWithConstraints(
            framesetter,
            CFRange {
                location: 0,
                length: attributed_string.length() as i64,
            },
            CFDictionaryRef::from(null_mut()),
            CGSize::new(f64::MAX, f64::MAX),
            null_mut(),
        );

        let rect = CGRect::new(
            CGPoint::ZERO,
            CGSize::new(size.width.ceil(), size.height.ceil()),
        );

        let path = CGPathCreateMutable();
        CGPathAddRect(path, null_mut(), rect);

        let frame = CTFramesetterCreateFrame(
            framesetter,
            CFRange {
                location: 0,
                length: 0,
            },
            path,
            null_mut(),
        );

        let mut raw_data = vec![0u8; rect.size.width as usize * rect.size.height as usize * 4];

        let bitmap_info = CGBitmapInfo(
            CGBitmapInfo::kCGBitmapByteOrder32Big.0
                | CGImageAlphaInfo::kCGImageAlphaPremultipliedLast.0,
        );

        let rgb_color_space = CGColorSpaceCreateDeviceRGB();

        let context = CGBitmapContextCreateWithData(
            raw_data.as_mut_ptr() as _,
            rect.size.width as usize,
            rect.size.height as usize,
            8,
            (rect.size.width * 4.0) as usize,
            rgb_color_space,
            bitmap_info.0,
            CGBitmapContextReleaseDataCallback::None,
            null_mut(),
        );

        CTFrameDraw(frame, context);

        (raw_data, rect)
    }

    unsafe fn has_color_glyphs(attributed_string: &Retained<NSMutableAttributedString>) -> bool {
        let typesetter =
            CTTypesetterCreateWithAttributedString(attributed_string.deref() as *const _ as _);
        let line = CTTypesetterCreateLine(
            typesetter,
            CFRange {
                location: 0,
                length: attributed_string.length() as i64,
            },
        );

        let runs = CTLineGetGlyphRuns(line);

        if runs.is_null() {
            return false;
        }

        let count = CFArrayGetCount(runs);

        for i in 0..count {
            let run = CFArrayGetValueAtIndex(runs, i);

            let attributes = CTRunGetAttributes(run);
            let font = CFDictionaryGetValue(attributes, kCTFontAttributeName);

            let traits = CTFontGetSymbolicTraits(font);

            if traits.contains(CTFontSymbolicTraits::kCTFontTraitColorGlyphs) {
                return true;
            }
        }

        false
    }
}
