use std::{
    cell::RefCell,
    ffi::c_void,
    ptr::{copy_nonoverlapping, NonNull},
    rc::Rc,
};

use objc2::{rc::Retained, runtime::ProtocolObject, sel};
use objc2_foundation::{ns_string, NSDefaultRunLoopMode, NSRunLoop};
use objc2_metal::*;
use objc2_quartz_core::{CADisplayLink, CAMetalDrawable};

use crate::{
    app::App,
    config::Config,
    geometry::{matrix::ortho, rect::Rect},
    platform::{
        aliases::{AnyText, AnyWindow},
        gfx::SpriteKind,
        text_cache::{AtlasDimensions, GlyphCacheResult, GlyphSpan, GlyphSpans},
    },
    ui::color::Color,
};

use super::{result::Result, text::Text, view::View};

const SHADER_CODE: &str = "
#include <metal_stdlib>

struct SceneProperties {
    metal::float4x4 projection;
    float2 texture_size;
};

struct VertexInput {
    float2 position;
    float4 color;
    float3 uv;
};

struct VertexOutput {
    float4 position [[position]];
    float4 color;
    float3 uv;
    float2 texture_size;
};

vertex VertexOutput vertex_main(
    device const SceneProperties& properties [[buffer(0)]],
    device const VertexInput* vertices [[buffer(1)]],
    uint vertex_idx [[vertex_id]]
) {
    VertexOutput output;
    VertexInput input = vertices[vertex_idx];
    output.position = properties.projection * float4(input.position.xy, 0, 1);
    output.color = input.color;
    output.uv = float3(input.uv.xy / properties.texture_size, input.uv.z);
    output.texture_size = properties.texture_size;
    return output;
}

fragment float4 fragment_main(
    VertexOutput input [[stage_in]],
    metal::texture2d<float> color_texture [[texture(0)]]
) {
    constexpr metal::sampler texture_sampler(metal::mag_filter::nearest, metal::min_filter::nearest);

    float4 color_sample = color_texture.sample(texture_sampler, input.uv.xy);

    float4 glyph_color = float4(input.color.rgb, color_sample.a);
    float4 color_glyph_color = color_sample;
    float4 rect_color = input.color;

    float4 colors[] = {glyph_color, color_glyph_color, rect_color};

    return colors[(int)input.uv.z];
}
";

#[derive(Copy, Clone)]
#[repr(C)]
struct SceneProperties {
    projection_matrix: [f32; 16],
    texture_size: [f32; 2],
}

#[derive(Copy, Clone)]
#[repr(C, align(16))]
struct VertexInput {
    position: [f32; 4],
    color: [f32; 4],
    uv: [f32; 3],
}

pub const PIXEL_FORMAT: MTLPixelFormat = MTLPixelFormat::BGRA8Unorm;

pub struct Gfx {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    view: Retained<View>,
    pub display_link: Retained<CADisplayLink>,

    vertices: Vec<VertexInput>,
    indices: Vec<u32>,

    buffers: Vec<Retained<ProtocolObject<dyn MTLBuffer>>>,
    next_buffer_index: usize,

    glyph_cache_result: GlyphCacheResult,
    drawable: Option<Retained<ProtocolObject<dyn CAMetalDrawable>>>,
    command_buffer: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    encoder: Option<Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>>,

    bounds: Rect,

    text: AnyText,
    texture: Option<Retained<ProtocolObject<dyn MTLTexture>>>,

    width: f32,
    height: f32,
    scale: f32,

    pub is_fullscreen: bool,
}

impl Gfx {
    pub fn new(
        app: Rc<RefCell<App>>,
        window: Rc<RefCell<AnyWindow>>,
        device: Retained<ProtocolObject<dyn MTLDevice>>,
        view: Retained<View>,
    ) -> Result<Self> {
        let command_queue = device
            .newCommandQueue()
            .expect("Failed to create a command queue.");

        let display_link = unsafe {
            let display_link = view.displayLinkWithTarget_selector(&view, sel!(update));
            display_link.addToRunLoop_forMode(&NSRunLoop::currentRunLoop(), NSDefaultRunLoopMode);

            display_link
        };

        let pipeline_descriptor = MTLRenderPipelineDescriptor::new();

        unsafe {
            pipeline_descriptor
                .colorAttachments()
                .objectAtIndexedSubscript(0)
                .setPixelFormat(PIXEL_FORMAT)
        }

        let library = device.newLibraryWithSource_options_error(ns_string!(SHADER_CODE), None);

        let library = match library {
            Ok(library) => library,
            Err(err) => {
                let localized_description = err.localizedDescription();
                panic!("Library error: {:?}", localized_description);
            }
        };

        let vertex_function = library.newFunctionWithName(ns_string!("vertex_main"));
        pipeline_descriptor.setVertexFunction(vertex_function.as_deref());

        let fragment_function = library.newFunctionWithName(ns_string!("fragment_main"));
        pipeline_descriptor.setFragmentFunction(fragment_function.as_deref());

        let color_attachment = unsafe { MTLRenderPipelineColorAttachmentDescriptor::new() };
        color_attachment.setBlendingEnabled(true);
        color_attachment.setRgbBlendOperation(MTLBlendOperation::Add);
        color_attachment.setAlphaBlendOperation(MTLBlendOperation::Add);
        color_attachment.setSourceRGBBlendFactor(MTLBlendFactor::SourceAlpha);
        color_attachment.setSourceAlphaBlendFactor(MTLBlendFactor::SourceAlpha);
        color_attachment.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        color_attachment.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        color_attachment.setPixelFormat(PIXEL_FORMAT);

        unsafe {
            pipeline_descriptor
                .colorAttachments()
                .setObject_atIndexedSubscript(Some(&color_attachment), 0);
        }

        let pipeline_state = device
            .newRenderPipelineStateWithDescriptor_error(&pipeline_descriptor)
            .expect("Failed to create a pipeline state.");

        let app = app.borrow();

        let Config {
            font, font_size, ..
        } = app.config();

        let scale = window.borrow().inner.scale as f32;

        let text = AnyText::new(font, |font_name| unsafe {
            Text::new(font_name, *font_size, scale)
        })?;

        let gfx = Gfx {
            device,
            command_queue,
            pipeline_state,
            view,
            display_link,

            vertices: Vec::new(),
            indices: Vec::new(),

            buffers: Vec::new(),
            next_buffer_index: 0,

            glyph_cache_result: GlyphCacheResult::Hit,
            drawable: None,
            command_buffer: None,
            encoder: None,

            bounds: Rect::zero(),

            text,
            texture: None,

            width: 0.0,
            height: 0.0,
            scale,

            is_fullscreen: false,
        };

        Ok(gfx)
    }

    pub fn resize(&mut self, width: f64, height: f64) -> Result<()> {
        self.width = width as f32;
        self.height = height as f32;

        Ok(())
    }

    pub fn update_font(&mut self, font_name: &str, font_size: f32, scale: f32) {
        self.scale = scale;

        self.text = AnyText::new(font_name, |font_name| unsafe {
            Text::new(font_name, font_size, scale)
        })
        .unwrap();
    }

    pub fn get_glyph_spans(&mut self, text: &str) -> GlyphSpans {
        let (spans, result) = self.text.get_glyph_spans(text);
        self.glyph_cache_result = self.glyph_cache_result.worse(result);

        spans
    }

    pub fn get_glyph_span(&mut self, index: usize) -> GlyphSpan {
        self.text.get_glyph_span(index)
    }

    fn handle_glyph_cache_result(&mut self) {
        let atlas = &mut self.text.cache.atlas;

        let glyph_cache_result = self.glyph_cache_result;
        self.glyph_cache_result = GlyphCacheResult::Hit;

        let (x, width) = match glyph_cache_result {
            GlyphCacheResult::Hit => return,
            GlyphCacheResult::Miss => (0, atlas.dimensions.width),
            GlyphCacheResult::Resize => {
                let texture_descriptor = unsafe {
                    MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                        MTLPixelFormat::RGBA8Unorm,
                        atlas.dimensions.width,
                        atlas.dimensions.height,
                        false,
                    )
                };

                let texture = self
                    .device
                    .newTextureWithDescriptor(&texture_descriptor)
                    .unwrap();

                self.texture = Some(texture);

                (0, atlas.dimensions.width)
            }
        };

        let texture = self.texture.as_ref().unwrap();

        let region = MTLRegion {
            origin: MTLOrigin { x, y: 0, z: 0 },
            size: MTLSize {
                width,
                height: atlas.dimensions.height,
                depth: 1,
            },
        };

        unsafe {
            texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                region,
                0,
                NonNull::new(atlas.data.as_mut_ptr())
                    .unwrap()
                    .cast::<c_void>(),
                atlas.dimensions.width * 4,
            );
        }
    }

    pub fn begin_frame(&mut self, clear_color: Color) {
        self.command_buffer = self.command_queue.commandBuffer();

        let Some(command_buffer) = self.command_buffer.as_ref() else {
            return;
        };

        self.drawable = unsafe { self.view.next_drawable() };
        let drawable = self.drawable.as_ref().unwrap();

        let color_attachment = MTLRenderPassColorAttachmentDescriptor::new();

        unsafe {
            color_attachment.setTexture(Some(&drawable.texture()));
        }

        color_attachment.setLoadAction(MTLLoadAction::Clear);
        color_attachment.setStoreAction(MTLStoreAction::Store);
        color_attachment.setClearColor(MTLClearColor {
            red: clear_color.r as f64 / 255.0f64,
            green: clear_color.g as f64 / 255.0f64,
            blue: clear_color.b as f64 / 255.0f64,
            alpha: clear_color.a as f64 / 255.0f64,
        });

        let pass_descriptor = MTLRenderPassDescriptor::renderPassDescriptor();

        unsafe {
            pass_descriptor
                .colorAttachments()
                .setObject_atIndexedSubscript(Some(&color_attachment), 0);
        }

        self.encoder = command_buffer.renderCommandEncoderWithDescriptor(&pass_descriptor);

        let Some(encoder) = self.encoder.as_ref() else {
            return;
        };

        encoder.setRenderPipelineState(&self.pipeline_state);
    }

    pub fn end_frame(&mut self) {
        let Some(command_buffer) = self.command_buffer.as_ref() else {
            return;
        };

        let Some(encoder) = self.encoder.as_ref() else {
            return;
        };

        let Some(drawable) = self.drawable.as_ref() else {
            return;
        };

        encoder.endEncoding();

        command_buffer.commit();
        drawable.present();

        self.encoder = None;
        self.command_buffer = None;
        self.drawable = None;

        self.next_buffer_index = 0;

        self.text.swap_caches();
    }

    pub fn begin(&mut self, bounds: Option<Rect>) {
        self.vertices.clear();
        self.indices.clear();

        if let Some(bounds) = bounds {
            self.bounds = bounds;
        } else {
            self.bounds = Rect::new(0.0, 0.0, self.width, self.height);
        }

        if !self.is_fullscreen {
            // MacOS draws a black border over the first pixel at the top of the window.
            self.bounds.y += 1.0;
        }
    }

    pub fn end(&mut self) {
        self.handle_glyph_cache_result();

        let Some(encoder) = self.encoder.as_ref() else {
            return;
        };

        encoder.setScissorRect(MTLScissorRect {
            x: self.bounds.x as usize,
            y: self.bounds.y as usize,
            width: self.bounds.width as usize,
            height: self.bounds.height as usize,
        });

        let projection = ortho(0.0, self.width, 0.0, self.height, -1.0, 1.0);

        let atlas_dimensions = self.atlas_dimensions();

        let scene_properties_data = &SceneProperties {
            projection_matrix: projection,
            texture_size: [
                atlas_dimensions.width as f32,
                atlas_dimensions.height as f32,
            ],
        };

        let scene_properties_bytes = NonNull::from(scene_properties_data);

        let buffer_index = self.next_buffer_index;
        self.next_buffer_index += 1;

        let Some(index_buffer) =
            Self::get_buffer_for_vec(&self.indices, &self.device, &mut self.buffers, buffer_index)
        else {
            return;
        };

        let buffer_index = self.next_buffer_index;
        self.next_buffer_index += 1;

        let Some(vertex_buffer) = Self::get_buffer_for_vec(
            &self.vertices,
            &self.device,
            &mut self.buffers,
            buffer_index,
        ) else {
            return;
        };

        unsafe {
            encoder.setVertexBytes_length_atIndex(
                scene_properties_bytes.cast::<c_void>(),
                size_of_val(scene_properties_data),
                0,
            );

            encoder.setVertexBuffer_offset_atIndex(Some(&vertex_buffer), 0, 1);

            if let Some(texture) = self.texture.as_ref() {
                encoder.setFragmentTexture_atIndex(Some(texture), 0);
            }
        }

        unsafe {
            encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(
                MTLPrimitiveType::Triangle,
                self.indices.len(),
                MTLIndexType::UInt32,
                &index_buffer,
                0,
            );
        }
    }

    fn get_buffer_for_vec<T>(
        vec: &[T],
        device: &Retained<ProtocolObject<dyn MTLDevice>>,
        buffers: &mut Vec<Retained<ProtocolObject<dyn MTLBuffer>>>,
        buffer_index: usize,
    ) -> Option<Retained<ProtocolObject<dyn MTLBuffer>>> {
        if vec.is_empty() {
            return None;
        }

        let mut buffer = buffers.get(buffer_index).cloned();

        if buffer
            .as_ref()
            .is_none_or(|buffer| buffer.length() < size_of_val(vec))
        {
            buffer = device.newBufferWithLength_options(
                size_of_val(vec),
                MTLResourceOptions::CPUCacheModeWriteCombined
                    | MTLResourceOptions::StorageModeShared,
            );
        }

        let buffer = buffer.unwrap();

        if buffer_index >= buffers.len() {
            buffers.push(buffer.clone());
            assert!(buffer_index < buffers.len(), "A buffer index was skipped");
        } else {
            buffers[buffer_index] = buffer.clone();
        }

        let contents = buffer.contents();

        unsafe {
            copy_nonoverlapping(vec.as_ptr(), contents.cast::<T>().as_ptr(), vec.len());
        }

        Some(buffer)
    }

    pub fn add_sprite(&mut self, src: Rect, dst: Rect, color: Color, kind: SpriteKind) {
        let vertex_count = self.vertices.len() as u32;

        self.indices.extend_from_slice(&[
            vertex_count,
            vertex_count + 1,
            vertex_count + 2,
            vertex_count,
            vertex_count + 2,
            vertex_count + 3,
        ]);

        let left = (dst.x + self.bounds.x).floor();
        let top = (dst.y + self.bounds.y).floor();
        let right = left + dst.width;
        let bottom = top + dst.height;

        let uv_left = src.x;
        let uv_right = src.x + src.width;
        let uv_top = src.y;
        let uv_bottom = src.y + src.height;

        let color = [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        ];

        let kind = kind as usize as f32;

        self.vertices.extend_from_slice(&[
            VertexInput {
                position: [left, top, 0.0, 0.0],
                color,
                uv: [uv_left, uv_top, kind],
            },
            VertexInput {
                position: [right, top, 0.0, 0.0],
                color,
                uv: [uv_right, uv_top, kind],
            },
            VertexInput {
                position: [right, bottom, 0.0, 0.0],
                color,
                uv: [uv_right, uv_bottom, kind],
            },
            VertexInput {
                position: [left, bottom, 0.0, 0.0],
                color,
                uv: [uv_left, uv_bottom, kind],
            },
        ]);
    }

    pub fn atlas_dimensions(&self) -> &AtlasDimensions {
        &self.text.cache.atlas.dimensions
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn view(&self) -> &Retained<View> {
        &self.view
    }
}
