use core::f64;
use std::{
    ffi::c_void,
    ptr::{null_mut, NonNull},
};

use super::result::Result;
use objc2::runtime::AnyObject;
use objc2_core_foundation::{
    kCFAllocatorDefault, CFDictionaryAddValue, CFDictionaryCreateMutable, CFDictionaryRef,
    CFNumberCreate, CFNumberType, CFRange, CFStringRef, CGPoint, CGRect, CGSize,
};
use objc2_core_graphics::{
    CGBitmapContextCreateWithData, CGBitmapContextReleaseDataCallback, CGBitmapInfo,
    CGColorSpaceCreateDeviceRGB, CGImageAlphaInfo, CGPathAddRect, CGPathCreateMutable,
};
use objc2_core_text::{
    kCTFontAttributeName, kCTFontFixedAdvanceAttribute, kCTFontNameAttribute,
    CTFontCreateWithFontDescriptor, CTFontCreateWithNameAndOptions,
    CTFontDescriptorCreateWithAttributes, CTFontGetAdvancesForGlyphs, CTFontGetAscent,
    CTFontGetDescent, CTFontGetLeading, CTFontOptions, CTFontOrientation, CTFrameDraw,
    CTFramesetterCreateFrame, CTFramesetterCreateWithAttributedString,
    CTFramesetterSuggestFrameSizeWithConstraints,
};
use objc2_foundation::{NSMutableAttributedString, NSRange, NSString};

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
    pub line_height: f32,
}

pub struct Text {}

impl Text {
    pub fn test(font_name: &str, font_size: f32, scale: f32) -> Atlas {
        const ATLAS_SIZE: usize = (b'~' - b' ') as usize;

        let mut atlas_text = String::with_capacity(ATLAS_SIZE);

        for c in '!'..='~' {
            atlas_text.push(c);
        }

        let mut attributed_string =
            NSMutableAttributedString::from_nsstring(&NSString::from_str(&atlas_text));

        let font_name = NSString::from_str(font_name);
        let font_name_cfstr = font_name.as_ref() as *const _ as CFStringRef;

        let glyph_width;
        let glyph_step_x;
        let line_height;

        unsafe {
            attributed_string.beginEditing();

            let font = CTFontCreateWithNameAndOptions(
                font_name_cfstr,
                font_size as f64,
                null_mut(),
                CTFontOptions::kCTFontOptionsDefault,
            ) as *mut AnyObject;

            let advance_glyphs = &[b'M' as u16];
            let mut advances = [CGSize::ZERO; 1];

            CTFontGetAdvancesForGlyphs(
                font as *mut c_void,
                CTFontOrientation::kCTFontOrientationHorizontal,
                NonNull::from(&advance_glyphs[0]),
                &mut advances[0],
                1,
            );

            line_height = CTFontGetAscent(font as _)
                + CTFontGetDescent(font as _)
                + CTFontGetLeading(font as _);

            let attributes =
                CFDictionaryCreateMutable(kCFAllocatorDefault, 2, null_mut(), null_mut());

            CFDictionaryAddValue(attributes, kCTFontNameAttribute, font_name_cfstr);

            glyph_width = advances[0].width.floor();
            glyph_step_x = advances[0].width.ceil() + 1.0;

            let mut advance = glyph_step_x;
            let advance = CFNumberCreate(
                kCFAllocatorDefault,
                CFNumberType::kCFNumberFloat64Type,
                &mut advance as *mut _ as _,
            );

            CFDictionaryAddValue(attributes, kCTFontFixedAdvanceAttribute, advance);

            let descriptor = CTFontDescriptorCreateWithAttributes(attributes);

            let font =
                CTFontCreateWithFontDescriptor(descriptor, 12.0, null_mut()) as *mut AnyObject;

            let font_attribute_name = kCTFontAttributeName as *const NSString;

            let attributed_string_len = attributed_string.length();
            attributed_string.addAttribute_value_range(
                &*font_attribute_name,
                &*font,
                NSRange::new(0, attributed_string_len),
            );

            attributed_string.endEditing();
        }

        let framesetter = unsafe {
            CTFramesetterCreateWithAttributedString(attributed_string.as_mut() as *mut _ as _)
        };

        let path = unsafe { CGPathCreateMutable() };

        let rect;

        unsafe {
            let fit_range = null_mut();
            let size = CTFramesetterSuggestFrameSizeWithConstraints(
                framesetter,
                CFRange {
                    location: 0,
                    length: attributed_string.length() as i64,
                },
                CFDictionaryRef::from(null_mut()),
                CGSize::new(f64::MAX, f64::MAX),
                fit_range,
            );

            rect = CGRect::new(
                CGPoint::ZERO,
                CGSize::new(size.width.ceil(), size.height.ceil()),
            );

            CGPathAddRect(path, null_mut(), rect);
        }

        let frame = unsafe {
            CTFramesetterCreateFrame(
                framesetter,
                CFRange {
                    location: 0,
                    length: 0,
                },
                path,
                null_mut(),
            )
        };

        let mut raw_data = Vec::new();
        raw_data.resize(
            rect.size.width as usize * rect.size.height as usize * 4,
            0u8,
        );

        let bitmap_info = CGBitmapInfo(
            CGBitmapInfo::kCGBitmapByteOrder32Big.0
                | CGImageAlphaInfo::kCGImageAlphaPremultipliedLast.0,
        );

        let rgb_color_space = unsafe { CGColorSpaceCreateDeviceRGB() };
        let context = unsafe {
            CGBitmapContextCreateWithData(
                raw_data.as_mut_ptr() as _,
                rect.size.width as usize,
                rect.size.height as usize,
                8,
                (rect.size.width * 4.0) as usize,
                rgb_color_space,
                bitmap_info.0,
                CGBitmapContextReleaseDataCallback::None,
                null_mut(),
            )
        };

        unsafe {
            CTFrameDraw(frame, context);
        }

        Atlas {
            data: raw_data,
            dimensions: AtlasDimensions {
                width: rect.size.width as usize,
                height: rect.size.height as usize,
                glyph_offset_x: 0.0,
                glyph_step_x: glyph_step_x as f32,
                glyph_width: glyph_width as f32,
                glyph_height: rect.size.height as f32,
                line_height: line_height.ceil() as f32,
            },
        }
    }

    pub fn new(font_name: &str, font_size: f32, scale: f32) -> Self {
        todo!()
    }

    pub fn generate_atlas(&mut self) -> Result<Atlas> {
        todo!()
    }
}
