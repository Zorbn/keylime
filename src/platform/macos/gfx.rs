use std::{
    borrow::Borrow,
    cell::RefCell,
    ffi::c_void,
    ptr::{copy_nonoverlapping, NonNull},
    rc::Rc,
};

use objc2::{
    declare_class, msg_send_id, mutability::MainThreadOnly, rc::Retained, runtime::ProtocolObject,
    ClassType, DeclaredClass,
};
use objc2_app_kit::{NSEvent, NSWindow};
use objc2_foundation::{ns_string, MainThreadMarker, NSRect};
use objc2_metal::{
    MTLBlendFactor, MTLBlendOperation, MTLBuffer, MTLClearColor, MTLCommandBuffer,
    MTLCommandEncoder, MTLCommandQueue, MTLCreateSystemDefaultDevice, MTLDevice, MTLIndexType,
    MTLLibrary, MTLPrimitiveType, MTLRenderCommandEncoder,
    MTLRenderPipelineColorAttachmentDescriptor, MTLRenderPipelineDescriptor,
    MTLRenderPipelineState, MTLResourceOptions, MTLScissorRect,
};
use objc2_metal_kit::{MTKView, MTKViewDelegate};

use crate::{
    geometry::{
        matrix::ortho,
        rect::Rect,
        side::{SIDE_BOTTOM, SIDE_LEFT, SIDE_RIGHT, SIDE_TOP},
    },
    ui::color::Color,
};

use super::{result::Result, window::Window};

const SHADER_CODE: &str = "
#include <metal_stdlib>

struct SceneProperties {
    metal::float4x4 projection;
};

struct VertexInput {
    metal::float4 position;
    metal::float4 color;
};

struct VertexOutput {
    metal::float4 position [[position]];
    metal::float4 color;
};

vertex VertexOutput vertex_main(
    device const SceneProperties& properties [[buffer(0)]],
    device const VertexInput* vertices [[buffer(1)]],
    uint vertex_idx [[vertex_id]]
) {
    VertexOutput out;
    VertexInput in = vertices[vertex_idx];
    out.position = properties.projection * metal::float4(in.position.xyz, 1);
    out.color = in.color;
    return out;
}

fragment metal::float4 fragment_main(VertexOutput in [[stage_in]]) {
    return in.color;
}
";

#[derive(Copy, Clone)]
#[repr(C)]
struct SceneProperties {
    projection_matrix: [f32; 16],
}

#[derive(Copy, Clone)]
#[repr(C)]
struct VertexInput {
    position: [f32; 4],
    color: [f32; 4],
}

struct KeylimeViewIvars {
    window: Rc<RefCell<Window>>,
}

declare_class!(
    struct KeylimeView;

    unsafe impl ClassType for KeylimeView {
        type Super = MTKView;
        type Mutability = MainThreadOnly;
        const NAME: &'static str = "KeylimeView";
    }

    impl DeclaredClass for KeylimeView {
        type Ivars = KeylimeViewIvars;
    }

    unsafe impl KeylimeView {
        #[method(acceptsFirstResponder)]
        #[allow(non_snake_case)]
        unsafe fn acceptsFirstResponder(&self) -> bool {
            // println!("called");
            true
        }

        #[method(keyDown:)]
        #[allow(non_snake_case)]
        unsafe fn keyDown(&self, event: &NSEvent) {
            let window = &mut *self.ivars().window.borrow_mut();

            window.handle_key_down(event);

            // println!("Key down");
        }

        #[method(mouseDown:)]
        #[allow(non_snake_case)]
        unsafe fn mouseDown(&self, event: &NSEvent) {
            // let click_count = unsafe { event.clickCount() };

            // println!("Clicked!: {:?}", click_count);
        }

        #[method(mouseMoved:)]
        #[allow(non_snake_case)]
        unsafe fn mouseMoved(&self, _event: &NSEvent) {
            // println!("mouse moved");
        }
    }
);

impl KeylimeView {
    fn new(
        window: Rc<RefCell<Window>>,
        mtm: MainThreadMarker,
        frame_rect: NSRect,
        device: Option<&ProtocolObject<dyn MTLDevice>>,
    ) -> Retained<Self> {
        let this = mtm.alloc();
        let this = this.set_ivars(KeylimeViewIvars { window });

        unsafe {
            msg_send_id![
                super(this),
                initWithFrame: frame_rect, device: device
            ]
        }
    }
}

// TODO: Copied from Windows impl.
const TAB_WIDTH: usize = 4;

pub struct Gfx {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    view: Retained<KeylimeView>,

    vertices: Vec<VertexInput>,
    indices: Vec<u32>,

    buffers: Vec<Retained<ProtocolObject<dyn MTLBuffer>>>,
    used_buffers: Vec<Retained<ProtocolObject<dyn MTLBuffer>>>,

    command_buffer: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    encoder: Option<Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>>,

    bounds: Rect,
}

impl Gfx {
    pub unsafe fn new(
        font_name: &str,
        font_size: f32,
        window: Rc<RefCell<Window>>,
        ns_window: &NSWindow,
        mtm: MainThreadMarker,
        delegate: &ProtocolObject<dyn MTKViewDelegate>,
    ) -> Result<Self> {
        let device = {
            let ptr = unsafe { MTLCreateSystemDefaultDevice() };
            unsafe { Retained::retain(ptr) }.expect("Failed to get default system device.")
        };

        let command_queue = device
            .newCommandQueue()
            .expect("Failed to create a command queue.");

        let frame_rect = ns_window.frame();
        let view = KeylimeView::new(window, mtm, frame_rect, Some(&device));

        let pipeline_descriptor = MTLRenderPipelineDescriptor::new();

        unsafe {
            pipeline_descriptor
                .colorAttachments()
                .objectAtIndexedSubscript(0)
                .setPixelFormat(view.colorPixelFormat())
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

        let color_attachment = MTLRenderPipelineColorAttachmentDescriptor::new();
        color_attachment.setBlendingEnabled(true);
        color_attachment.setRgbBlendOperation(MTLBlendOperation::Add);
        color_attachment.setAlphaBlendOperation(MTLBlendOperation::Add);
        color_attachment.setSourceRGBBlendFactor(MTLBlendFactor::SourceAlpha);
        color_attachment.setSourceAlphaBlendFactor(MTLBlendFactor::SourceAlpha);
        color_attachment.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        color_attachment.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        color_attachment.setPixelFormat(view.colorPixelFormat());

        pipeline_descriptor
            .colorAttachments()
            .setObject_atIndexedSubscript(Some(&color_attachment), 0);

        let pipeline_state = device
            .newRenderPipelineStateWithDescriptor_error(&pipeline_descriptor)
            .expect("Failed to create a pipeline state.");

        unsafe {
            view.setDelegate(Some(delegate));
        }

        ns_window.setContentView(Some(&view));

        Ok(Gfx {
            device,
            command_queue,
            pipeline_state,
            view,

            vertices: Vec::new(),
            indices: Vec::new(),

            buffers: Vec::new(),
            used_buffers: Vec::new(),

            command_buffer: None,
            encoder: None,

            bounds: Rect::zero(),
        })
    }

    pub unsafe fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        Ok(())
    }

    pub fn update_font(&mut self, font_name: &str, font_size: f32, scale: f32) {}

    pub fn begin_frame(&mut self, clear_color: Color) {
        unsafe {
            self.view.setClearColor(MTLClearColor {
                red: clear_color.r as f64 / 255.0f64,
                green: clear_color.g as f64 / 255.0f64,
                blue: clear_color.b as f64 / 255.0f64,
                alpha: clear_color.a as f64 / 255.0f64,
            });
        }

        self.command_buffer = self.command_queue.commandBuffer();

        let Some(command_buffer) = self.command_buffer.as_ref() else {
            return;
        };

        let Some(pass_descriptor) = (unsafe { self.view.currentRenderPassDescriptor() }) else {
            return;
        };

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

        let Some(current_drawable) = (unsafe { self.view.currentDrawable() }) else {
            return;
        };

        encoder.endEncoding();

        command_buffer.presentDrawable(ProtocolObject::from_ref(&*current_drawable));
        command_buffer.commit();

        self.encoder = None;
        self.command_buffer = None;

        self.buffers.extend(self.used_buffers.drain(..));
    }

    pub fn begin(&mut self, bounds: Option<Rect>) {
        self.vertices.clear();
        self.indices.clear();

        if let Some(bounds) = bounds {
            self.bounds = bounds;
        } else {
            self.bounds = Rect::new(0.0, 0.0, 768.0, 768.0);
        }
    }

    pub fn end(&mut self) {
        let Some(encoder) = self.encoder.as_ref() else {
            return;
        };

        encoder.setScissorRect(MTLScissorRect {
            x: self.bounds.x as usize,
            y: self.bounds.y as usize,
            width: self.bounds.width as usize,
            height: self.bounds.height as usize,
        });

        let projection = ortho(0.0, 768.0, 0.0, 768.0, -1.0, 1.0);

        let scene_properties_data = &SceneProperties {
            projection_matrix: projection,
        };

        let scene_properties_bytes = NonNull::from(scene_properties_data);

        let Some(index_buffer) = Self::get_buffer_for_vec(
            &self.indices,
            &self.device,
            &mut self.buffers,
            &mut self.used_buffers,
        ) else {
            return;
        };

        let Some(vertex_buffer) = Self::get_buffer_for_vec(
            &self.vertices,
            &self.device,
            &mut self.buffers,
            &mut self.used_buffers,
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
        used_buffers: &mut Vec<Retained<ProtocolObject<dyn MTLBuffer>>>,
    ) -> Option<Retained<ProtocolObject<dyn MTLBuffer>>> {
        let mut buffer = buffers.pop();

        if !vec.is_empty()
            && buffer
                .as_ref()
                .is_none_or(|buffer| buffer.length() < vec.len() * size_of::<T>())
        {
            buffer = device.newBufferWithLength_options(
                vec.len() * size_of::<T>(),
                MTLResourceOptions::MTLResourceCPUCacheModeWriteCombined
                    | MTLResourceOptions::MTLResourceStorageModeShared,
            );
        }

        if let Some(buffer) = &buffer {
            used_buffers.push(buffer.clone());

            let contents = buffer.contents();

            unsafe {
                copy_nonoverlapping(vec.as_ptr(), contents.cast::<T>().as_ptr(), vec.len());
            }
        }

        buffer
    }

    pub fn add_sprite(&mut self, src: Rect, dst: Rect, color: Color) {
        let vertex_count = self.vertices.len() as u32;
        self.indices.extend_from_slice(&[
            vertex_count,
            vertex_count + 1,
            vertex_count + 2,
            vertex_count,
            vertex_count + 2,
            vertex_count + 3,
        ]);

        let left = dst.x + self.bounds.x;
        let top = dst.y + self.bounds.y;
        let right = left + dst.width;
        let bottom = top + dst.height;

        let color = [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        ];

        self.vertices.extend_from_slice(&[
            VertexInput {
                position: [left, top, 0.0, 0.0],
                color,
            },
            VertexInput {
                position: [right, top, 0.0, 0.0],
                color,
            },
            VertexInput {
                position: [right, bottom, 0.0, 0.0],
                color,
            },
            VertexInput {
                position: [left, bottom, 0.0, 0.0],
                color,
            },
        ]);
    }

    pub fn measure_text(text: impl IntoIterator<Item = impl Borrow<char>>) -> isize {
        // TODO: Copied from Windows impl.
        let mut width = 0isize;

        for c in text.into_iter() {
            let c = *c.borrow();

            width += if c == '\t' { TAB_WIDTH as isize } else { 1 };
        }

        width
    }

    pub fn find_x_for_visual_x(
        text: impl IntoIterator<Item = impl Borrow<char>>,
        visual_x: isize,
    ) -> isize {
        0
    }

    pub fn get_char_width(c: char) -> isize {
        // TODO: Copied from Windows impl.
        match c {
            '\t' => TAB_WIDTH as isize,
            _ => 1,
        }
    }

    pub fn add_text(
        &mut self,
        text: impl IntoIterator<Item = impl Borrow<char>>,
        x: f32,
        y: f32,
        color: Color,
    ) -> isize {
        // TODO: Copied from Windows impl.

        let min_char = b' ' as u32;
        let max_char = b'~' as u32;

        // let AtlasDimensions {
        //     width,
        //     glyph_offset_x,
        //     glyph_step_x,
        //     glyph_width,
        //     glyph_height,
        //     ..
        // } = self.atlas_dimensions;
        let width = 1.0;
        let glyph_offset_x = 0.0;
        let glyph_step_x = 8.0;
        let glyph_width = 8.0;
        let glyph_height = 10.0;

        let mut i = 0;

        for c in text.into_iter() {
            let c = *c.borrow();

            let char_index = c as u32;

            if char_index <= min_char || char_index > max_char {
                i += Self::get_char_width(c);
                continue;
            }

            let atlas_char_index = char_index - min_char - 1;

            let mut source_x =
                (glyph_step_x * atlas_char_index as f32 - glyph_offset_x) / width as f32;
            let mut source_width = glyph_step_x / width as f32;

            let mut destination_x = x + i as f32 * glyph_width;
            let mut destination_width = glyph_step_x;

            // DirectWrite might press the first character in the atlas right up against the left edge (eg. the exclamation point),
            // so we'll just shift it back to the center when rendering if necessary.
            if source_x < 0.0 {
                destination_width += source_x * width as f32;
                destination_x -= source_x * width as f32;

                source_width += source_x;
                source_x = 0.0;
            }

            self.add_sprite(
                Rect::new(source_x, 0.0, source_width, 1.0),
                Rect::new(destination_x, y, destination_width, glyph_height),
                color,
            );

            i += Self::get_char_width(c);
        }

        i
    }

    pub fn add_bordered_rect(&mut self, rect: Rect, sides: u8, color: Color, border_color: Color) {
        // TODO: Copied from Windows impl.
        let border_width = self.border_width();

        self.add_rect(rect, border_color);

        let left = rect.x
            + if sides & SIDE_LEFT != 0 {
                border_width
            } else {
                0.0
            };

        let right = rect.x + rect.width
            - if sides & SIDE_RIGHT != 0 {
                border_width
            } else {
                0.0
            };

        let top = rect.y
            + if sides & SIDE_TOP != 0 {
                border_width
            } else {
                0.0
            };

        let bottom = rect.y + rect.height
            - if sides & SIDE_BOTTOM != 0 {
                border_width
            } else {
                0.0
            };

        self.add_rect(Rect::new(left, top, right - left, bottom - top), color);
    }

    pub fn add_rect(&mut self, rect: Rect, color: Color) {
        // TODO: Copied from Windows impl.
        self.add_sprite(Rect::new(-1.0, -1.0, -1.0, -1.0), rect, color);
    }

    pub fn glyph_width(&self) -> f32 {
        8.0
    }

    pub fn glyph_height(&self) -> f32 {
        10.0
    }

    pub fn line_height(&self) -> f32 {
        12.0
    }

    pub fn line_padding(&self) -> f32 {
        (self.line_height() - self.glyph_height()) / 2.0
    }

    pub fn border_width(&self) -> f32 {
        1.0
    }

    pub fn width(&self) -> f32 {
        768.0
    }

    pub fn height(&self) -> f32 {
        768.0
    }

    pub fn tab_height(&self) -> f32 {
        self.line_height() * 1.25
    }

    pub fn tab_padding_y(&self) -> f32 {
        (self.tab_height() - self.line_height()) * 0.75
    }

    pub fn height_lines(&self) -> isize {
        (self.height() / self.line_height()) as isize
    }
}
