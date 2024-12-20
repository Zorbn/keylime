#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    cell::{OnceCell, RefCell},
    ffi::c_void,
    ptr::{copy_nonoverlapping, NonNull},
    rc::Rc,
};

use objc2::{
    define_class, msg_send, msg_send_id, rc::Retained, runtime::ProtocolObject, sel, DeclaredClass,
    MainThreadOnly,
};
use objc2_app_kit::{
    NSEvent, NSView, NSViewLayerContentsPlacement, NSViewLayerContentsRedrawPolicy, NSWindow,
};
use objc2_core_foundation::CGSize;
use objc2_foundation::{
    ns_string, MainThreadMarker, NSDefaultRunLoopMode, NSObjectNSThreadPerformAdditions,
    NSObjectProtocol, NSRect, NSRunLoop, NSSize,
};
use objc2_metal::*;
use objc2_quartz_core::{
    CAAutoresizingMask, CADisplayLink, CALayer, CALayerDelegate, CAMetalDrawable, CAMetalLayer,
};

use crate::{
    app::App,
    config::Config,
    geometry::{matrix::ortho, rect::Rect},
    platform::{aliases::AnyWindow, text::AtlasDimensions},
    ui::color::Color,
};

use super::{result::Result, text::Text};

const SHADER_CODE: &str = "
#include <metal_stdlib>

struct SceneProperties {
    metal::float4x4 projection;
};

struct VertexInput {
    metal::float4 position;
    metal::float4 color;
    metal::float4 uv;
};

struct VertexOutput {
    metal::float4 position [[position]];
    metal::float4 color;
    metal::float2 uv;
};

vertex VertexOutput vertex_main(
    device const SceneProperties& properties [[buffer(0)]],
    device const VertexInput* vertices [[buffer(1)]],
    uint vertex_idx [[vertex_id]]
) {
    VertexOutput output;
    VertexInput input = vertices[vertex_idx];
    output.position = properties.projection * metal::float4(input.position.xyz, 1);
    output.color = input.color;
    output.uv = input.uv.xy;
    return output;
}

fragment metal::float4 fragment_main(
    VertexOutput input [[stage_in]],
    metal::texture2d<float> color_texture [[texture(0)]]
) {
    constexpr metal::sampler texture_sampler(metal::mag_filter::nearest, metal::min_filter::nearest);

    const metal::float4 color_sample = color_texture.sample(texture_sampler, input.uv);

    return input.uv.y < 0 ?
        input.color :
        float4(input.color.rgb, color_sample.a);
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
    uv: [f32; 4],
}

macro_rules! handle_event {
    ($handler:ident, $self:expr, $event:expr $(, $args:expr)*) => {
        {
            let window = &mut *$self.ivars().window.borrow_mut();
            window.inner.$handler($event, $($args), *);
        }

        $self.update();
    };
}

pub struct ViewIvars {
    app: Rc<RefCell<App>>,
    window: Rc<RefCell<AnyWindow>>,
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    metal_layer: OnceCell<Retained<CAMetalLayer>>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "View"]
    #[ivars = ViewIvars]
    pub struct View;

    unsafe impl View {
        #[method_id(makeBackingLayer)]
        unsafe fn make_backing_layer(&self) -> Retained<CALayer> {
            let metal_layer = unsafe { CAMetalLayer::new() };

            unsafe {
                metal_layer.setPixelFormat(PIXEL_FORMAT);
                metal_layer.setDevice(Some(&self.ivars().device));

                let protocol_object = ProtocolObject::from_ref(self);
                metal_layer.setDelegate(Some(&protocol_object));

                metal_layer.setAllowsNextDrawableTimeout(false);

                metal_layer.setAutoresizingMask(
                    CAAutoresizingMask::kCALayerWidthSizable
                        | CAAutoresizingMask::kCALayerHeightSizable,
                );

                metal_layer.setNeedsDisplayOnBoundsChange(true);
            }

            self.ivars().metal_layer.set(metal_layer.clone()).unwrap();

            let layer = Retained::<CALayer>::from(&metal_layer);
            layer
        }

        #[method(setFrameSize:)]
        unsafe fn set_frame_size(&self, new_size: NSSize) {
            unsafe {
                let _: () = msg_send![super(self), setFrameSize: new_size];
            }

            let metal_layer = self.ivars().metal_layer.get().unwrap();

            let window = &mut *self.ivars().window.borrow_mut();
            let app = &*self.ivars().app.borrow();

            let scale = window.inner.ns_window.backingScaleFactor();
            let new_size = CGSize::new(new_size.width * scale, new_size.height * scale);

            metal_layer.setContentsScale(scale);

            unsafe {
                metal_layer.setDrawableSize(new_size);
            }

            window.inner.resize(new_size.width, new_size.height, app);
        }

        #[method(viewWillStartLiveResize)]
        unsafe fn view_will_start_live_resize(&self) {
            let metal_layer = self.ivars().metal_layer.get().unwrap();

            unsafe {
                metal_layer.setPresentsWithTransaction(true);
            }
        }

        #[method(viewDidEndLiveResize)]
        unsafe fn view_did_end_live_resize(&self) {
            let metal_layer = self.ivars().metal_layer.get().unwrap();

            unsafe {
                metal_layer.setPresentsWithTransaction(false);
            }
        }

        #[method(viewDidChangeBackingProperties)]
        unsafe fn view_did_change_backing_properties(&self) {
            let metal_layer = self.ivars().metal_layer.get().unwrap();

            let scale = metal_layer.contentsScale();
            let size = unsafe { metal_layer.drawableSize() };
            let size = CGSize::new(size.width / scale, size.height / scale);

            unsafe {
                self.setFrameSize(size);
            }

            let mut window = self.ivars().window.borrow_mut();
            let view = window.gfx().inner.view();

            unsafe {
                view.setNeedsDisplay(true);
            }
        }

        #[method(acceptsFirstResponder)]
        unsafe fn accepts_first_responder(&self) -> bool {
            true
        }

        #[method(keyDown:)]
        unsafe fn key_down(&self, event: &NSEvent) {
            handle_event!(handle_key_down, self, event);
        }

        #[method(mouseDown:)]
        unsafe fn mouse_down(&self, event: &NSEvent) {
            handle_event!(handle_mouse_down, self, event, false);
        }

        #[method(rightMouseDown:)]
        unsafe fn right_mouse_down(&self, event: &NSEvent) {
            handle_event!(handle_mouse_down, self, event, false);
        }

        #[method(otherMouseDown:)]
        unsafe fn other_mouse_down(&self, event: &NSEvent) {
            handle_event!(handle_mouse_down, self, event, false);
        }

        #[method(mouseUp:)]
        unsafe fn mouse_up(&self, event: &NSEvent) {
            handle_event!(handle_mouse_up, self, event);
        }

        #[method(rightMouseUp:)]
        unsafe fn right_mouse_up(&self, event: &NSEvent) {
            handle_event!(handle_mouse_up, self, event);
        }

        #[method(otherMouseUp:)]
        unsafe fn other_mouse_up(&self, event: &NSEvent) {
            handle_event!(handle_mouse_up, self, event);
        }

        #[method(mouseDragged:)]
        unsafe fn mouse_dragged(&self, event: &NSEvent) {
            handle_event!(handle_mouse_down, self, event, true);
        }

        #[method(mouseMoved:)]
        unsafe fn mouse_moved(&self, event: &NSEvent) {
            handle_event!(handle_mouse_down, self, event, true);
        }

        #[method(scrollWheel:)]
        unsafe fn scroll_wheel(&self, event: &NSEvent) {
            handle_event!(handle_scroll_wheel, self, event);
        }

        #[method(update)]
        fn update_objc(&self) {
            self.update();
        }
    }

    unsafe impl NSObjectProtocol for View {}

    unsafe impl CALayerDelegate for View {
        #[method(displayLayer:)]
        unsafe fn display_layer(&self, _layer: &CALayer) {
            let Ok(mut window) = self.ivars().window.try_borrow_mut() else {
                return;
            };

            let Ok(mut app) = self.ivars().app.try_borrow_mut() else {
                return;
            };

            let window = &mut *window;

            app.draw(window);

            if !window.inner.was_shown {
                window.inner.ns_window.makeKeyAndOrderFront(None);
                window.inner.was_shown = true;
            }

            let gfx = window.gfx();

            unsafe {
                gfx.inner.display_link.setPaused(!app.is_animating());
            }
        }
    }
);

const PIXEL_FORMAT: MTLPixelFormat = MTLPixelFormat::BGRA8Unorm;

impl View {
    fn new(
        app: Rc<RefCell<App>>,
        window: Rc<RefCell<AnyWindow>>,
        mtm: MainThreadMarker,
        frame_rect: NSRect,
        device: Retained<ProtocolObject<dyn MTLDevice>>,
    ) -> Retained<Self> {
        let this = mtm.alloc();
        let this = this.set_ivars(ViewIvars {
            app,
            window,
            device,
            metal_layer: OnceCell::new(),
        });

        let view: Retained<View> = unsafe {
            msg_send_id![
                super(this),
                initWithFrame: frame_rect
            ]
        };

        view.setWantsLayer(true);

        unsafe {
            view.setLayerContentsRedrawPolicy(
                NSViewLayerContentsRedrawPolicy::NSViewLayerContentsRedrawDuringViewResize,
            );

            view.setLayerContentsPlacement(NSViewLayerContentsPlacement::ScaleAxesIndependently);
        }

        view
    }

    fn update(&self) {
        let Ok(mut window) = self.ivars().window.try_borrow_mut() else {
            return;
        };

        let Ok(mut app) = self.ivars().app.try_borrow_mut() else {
            return;
        };

        let window = &mut *window;

        let (time, dt) = window.inner.get_time(app.is_animating());
        app.update(window, time, dt);

        let (files, ptys) = app.files_and_ptys();
        window.inner.update(files, ptys);

        unsafe {
            self.setNeedsDisplay(true);
        }
    }
}

// SAFETY: It's only ok to use the view to trigger an update,
// and only once an NSThread has been created to signal to Cocoa
// that multi-threading is used.
unsafe impl Send for ViewRef {}
unsafe impl Sync for ViewRef {}

pub struct ViewRef {
    inner: Retained<View>,
}

impl ViewRef {
    pub fn new(inner: &Retained<View>) -> Self {
        Self {
            inner: inner.clone(),
        }
    }

    pub unsafe fn set_needs_display(&self) {
        unsafe {
            self.inner
                .performSelectorOnMainThread_withObject_waitUntilDone(sel!(update), None, false);
        }
    }
}

pub struct Gfx {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    view: Retained<View>,
    display_link: Retained<CADisplayLink>,

    vertices: Vec<VertexInput>,
    indices: Vec<u32>,

    buffers: Vec<Retained<ProtocolObject<dyn MTLBuffer>>>,
    next_buffer_index: usize,

    drawable: Option<Retained<ProtocolObject<dyn CAMetalDrawable>>>,
    command_buffer: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    encoder: Option<Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>>,

    bounds: Rect,

    atlas_dimensions: AtlasDimensions,
    texture: Retained<ProtocolObject<dyn MTLTexture>>,

    width: f32,
    height: f32,
    scale: f32,

    pub is_fullscreen: bool,
}

impl Gfx {
    pub unsafe fn new(
        app: Rc<RefCell<App>>,
        window: Rc<RefCell<AnyWindow>>,
        ns_window: &NSWindow,
        mtm: MainThreadMarker,
    ) -> Result<Self> {
        let scale = window.borrow().inner.scale();

        let device = {
            let ptr = unsafe { MTLCreateSystemDefaultDevice() };
            unsafe { Retained::retain(ptr) }.expect("Failed to get default system device.")
        };

        let command_queue = device
            .newCommandQueue()
            .expect("Failed to create a command queue.");

        let frame_rect = ns_window.frame();

        let view = View::new(app.clone(), window, mtm, frame_rect, device.clone());

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

        let (texture, atlas_dimensions) =
            Self::create_atlas_texture(&device, font, *font_size, scale)?;

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

            drawable: None,
            command_buffer: None,
            encoder: None,

            bounds: Rect::zero(),

            atlas_dimensions,
            texture,

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

        if let Ok(result) = Self::create_atlas_texture(&self.device, font_name, font_size, scale) {
            (self.texture, self.atlas_dimensions) = result;
        }
    }

    fn create_atlas_texture(
        device: &Retained<ProtocolObject<dyn MTLDevice>>,
        font_name: &str,
        font_size: f32,
        scale: f32,
    ) -> Result<(Retained<ProtocolObject<dyn MTLTexture>>, AtlasDimensions)> {
        let mut text = Text::new(font_name, font_size, scale);
        let mut atlas = text.generate_atlas()?;

        let texture_descriptor = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                MTLPixelFormat::RGBA8Unorm,
                atlas.dimensions.width,
                atlas.dimensions.height,
                false,
            )
        };

        let texture = device
            .newTextureWithDescriptor(&texture_descriptor)
            .unwrap();

        let region = MTLRegion {
            origin: MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width: atlas.dimensions.width,
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

        Ok((texture, atlas.dimensions))
    }

    pub fn begin_frame(&mut self, clear_color: Color) {
        let metal_layer = self.view.ivars().metal_layer.get().unwrap();

        self.command_buffer = self.command_queue.commandBuffer();

        let Some(command_buffer) = self.command_buffer.as_ref() else {
            return;
        };

        self.drawable = unsafe { metal_layer.nextDrawable() };
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

        let scene_properties_data = &SceneProperties {
            projection_matrix: projection,
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

            encoder.setFragmentTexture_atIndex(Some(&self.texture), 0);
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
            .is_none_or(|buffer| buffer.length() < vec.len() * size_of::<T>())
        {
            buffer = device.newBufferWithLength_options(
                vec.len() * size_of::<T>(),
                MTLResourceOptions::MTLResourceCPUCacheModeWriteCombined
                    | MTLResourceOptions::MTLResourceStorageModeShared,
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

        self.vertices.extend_from_slice(&[
            VertexInput {
                position: [left, top, 0.0, 0.0],
                color,
                uv: [uv_left, uv_top, 0.0, 0.0],
            },
            VertexInput {
                position: [right, top, 0.0, 0.0],
                color,
                uv: [uv_right, uv_top, 0.0, 0.0],
            },
            VertexInput {
                position: [right, bottom, 0.0, 0.0],
                color,
                uv: [uv_right, uv_bottom, 0.0, 0.0],
            },
            VertexInput {
                position: [left, bottom, 0.0, 0.0],
                color,
                uv: [uv_left, uv_bottom, 0.0, 0.0],
            },
        ]);
    }

    pub fn atlas_dimensions(&self) -> &AtlasDimensions {
        &self.atlas_dimensions
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
