#![deny(unsafe_op_in_unsafe_fn)]

use std::{cell::OnceCell, ffi::c_void, path::Path, ptr::NonNull};

use objc2::{
    declare_class, msg_send, msg_send_id, mutability::MainThreadOnly, rc::Retained,
    runtime::ProtocolObject, ClassType, DeclaredClass,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSDate, NSNotification, NSObject, NSObjectProtocol, NSPoint,
    NSRect, NSSize,
};
use objc2_metal::{
    MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue, MTLCreateSystemDefaultDevice, MTLDevice,
    MTLLibrary, MTLPackedFloat3, MTLPrimitiveType, MTLRenderCommandEncoder,
    MTLRenderPipelineDescriptor, MTLRenderPipelineState,
};
use objc2_metal_kit::{MTKView, MTKViewDelegate};

use crate::{
    app::App,
    input::{
        input_handlers::{CharHandler, KeybindHandler, MouseScrollHandler, MousebindHandler},
        keybind::Keybind,
        mouse_scroll::MouseScroll,
        mousebind::Mousebind,
    },
};

use super::{file_watcher::FileWatcher, gfx::Gfx, pty::Pty, result::Result};

const SHADER_CODE: &str = "
#include <metal_stdlib>

struct SceneProperties {
    float time;
};

struct VertexInput {
    metal::packed_float3 position;
    metal::packed_float3 color;
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
    out.position =
        metal::float4(
            metal::float2x2(
                metal::cos(properties.time), -metal::sin(properties.time),
                metal::sin(properties.time),  metal::cos(properties.time)
            ) * in.position.xy,
            in.position.z,
            1);
    out.color = metal::float4(in.color, 1);
    return out;
}

fragment metal::float4 fragment_main(VertexOutput in [[stage_in]]) {
    return in.color;
}
";

#[derive(Copy, Clone)]
#[repr(C)]
struct SceneProperties {
    time: f32,
}

#[derive(Copy, Clone)]
#[repr(C)]
struct VertexInput {
    position: MTLPackedFloat3,
    color: MTLPackedFloat3,
}

macro_rules! idcell {
    ($name:ident => $this:expr) => {
        $this.ivars().$name.set($name).expect(&format!(
            "ivar should not be initialized: `{}`",
            stringify!($name)
        ));
    };
    ($name:ident <= $this:expr) => {
        let Some($name) = $this.ivars().$name.get() else {
            unreachable!("ivar should be initialized: `{}`", stringify!($name));
        };
    };
}

struct Ivars {
    start_date: Retained<NSDate>,
    command_queue: OnceCell<Retained<ProtocolObject<dyn MTLCommandQueue>>>,
    pipeline_state: OnceCell<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>,
    window: OnceCell<Retained<NSWindow>>,
}

declare_class!(
    struct Delegate;

    unsafe impl ClassType for Delegate {
        type Super = NSObject;
        type Mutability = MainThreadOnly;
        const NAME: &'static str = "Delegate";
    }

    impl DeclaredClass for Delegate {
        type Ivars = Ivars;
    }

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl NSApplicationDelegate for Delegate {
        #[method(applicationDidFinishLaunching:)]
        #[allow(non_snake_case)]
        unsafe fn applicationDidFinishLaunching(&self, _notification: &NSNotification) {
            let mtm = MainThreadMarker::from(self);

            let window = {
                let content_rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(768.0, 768.0));

                let style = NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Resizable
                    | NSWindowStyleMask::Miniaturizable
                    | NSWindowStyleMask::Titled;

                let backing_store_type = NSBackingStoreType::NSBackingStoreBuffered;
                let flag = false;

                unsafe {
                    NSWindow::initWithContentRect_styleMask_backing_defer(
                        mtm.alloc(),
                        content_rect,
                        style,
                        backing_store_type,
                        flag,
                    )
                }
            };

            let device = {
                let ptr = unsafe { MTLCreateSystemDefaultDevice() };
                unsafe { Retained::retain(ptr) }.expect("Failed to get default system device.")
            };

            let command_queue = device.newCommandQueue().expect("Failed to create a command queue.");

            let mtk_view = {
                let frame_rect = window.frame();
                unsafe { MTKView::initWithFrame_device(mtm.alloc(), frame_rect, Some(&device)) }
            };

            let pipeline_descriptor = MTLRenderPipelineDescriptor::new();

            unsafe {
                pipeline_descriptor.colorAttachments().objectAtIndexedSubscript(0).setPixelFormat(mtk_view.colorPixelFormat())
            }

            let library = device.newLibraryWithSource_options_error(ns_string!(SHADER_CODE), None).expect("Failed to create library.");

            let vertex_function = library.newFunctionWithName(ns_string!("vertex_main"));
            pipeline_descriptor.setVertexFunction(vertex_function.as_deref());

            let fragment_function = library.newFunctionWithName(ns_string!("fragment_main"));
            pipeline_descriptor.setFragmentFunction(fragment_function.as_deref());

            let pipeline_state = device.newRenderPipelineStateWithDescriptor_error(&pipeline_descriptor).expect("Failed to create a pipeline state.");

            unsafe {
                let object = ProtocolObject::from_ref(self);
                mtk_view.setDelegate(Some(object));
            }

            window.setContentView(Some(&mtk_view));
            window.center();
            window.setTitle(ns_string!("Keylime"));
            window.makeKeyAndOrderFront(None);

            idcell!(command_queue => self);
            idcell!(pipeline_state => self);
            idcell!(window => self);

            unsafe {
                let app: &mut NSApplication = msg_send![_notification, object];
                app.activate();
            }
        }

        #[method(applicationShouldTerminateAfterLastWindowClosed:)]
        #[allow(non_snake_case)]
        unsafe fn applicationShouldTerminateAfterLastWindowClosed(&self, _sender: &NSApplication) -> bool {
            true
        }
    }

    unsafe impl MTKViewDelegate for Delegate {
        #[method(drawInMTKView:)]
        #[allow(non_snake_case)]
        unsafe fn drawInMTKView(&self, mtk_view: &MTKView) {
            idcell!(command_queue <= self);
            idcell!(pipeline_state <= self);

            let Some(current_drawable) = (unsafe { mtk_view.currentDrawable() }) else {
                return;
            };

            let Some(command_buffer) = command_queue.commandBuffer() else {
                return;
            };

            let Some(pass_descriptor) = (unsafe { mtk_view.currentRenderPassDescriptor() }) else {
                return;
            };

            let Some(encoder) = command_buffer.renderCommandEncoderWithDescriptor(&pass_descriptor) else {
                return;
            };

            let scene_properties_data = &SceneProperties {
                time: unsafe { self.ivars().start_date.timeIntervalSinceNow() } as f32,
            };

            let scene_properties_bytes = NonNull::from(scene_properties_data);

            unsafe {
                encoder.setVertexBytes_length_atIndex(scene_properties_bytes.cast::<c_void>(), size_of_val(scene_properties_data), 0);
            }

            let vertex_input_data: &[VertexInput] = &[
                VertexInput {
                    position: MTLPackedFloat3 {
                        x: -f32::sqrt(3.0) / 4.0,
                        y: -0.25,
                        z: 0.,
                    },
                    color: MTLPackedFloat3 {
                        x: 1.,
                        y: 0.,
                        z: 0.,
                    },
                },
                VertexInput {
                    position: MTLPackedFloat3 {
                        x: f32::sqrt(3.0) / 4.0,
                        y: -0.25,
                        z: 0.,
                    },
                    color: MTLPackedFloat3 {
                        x: 0.,
                        y: 1.,
                        z: 0.,
                    },
                },
                VertexInput {
                    position: MTLPackedFloat3 {
                        x: 0.,
                        y: 0.5,
                        z: 0.,
                    },
                    color: MTLPackedFloat3 {
                        x: 0.,
                        y: 0.,
                        z: 1.,
                    },
                },
            ];

            let vertex_input_bytes = NonNull::from(vertex_input_data);

            unsafe {
                encoder.setVertexBytes_length_atIndex(
                    vertex_input_bytes.cast::<c_void>(),
                    size_of_val(vertex_input_data),
                    1,
                );
            }

            encoder.setRenderPipelineState(pipeline_state);

            unsafe {
                encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3);
            }

            encoder.endEncoding();

            command_buffer.presentDrawable(ProtocolObject::from_ref(&*current_drawable));
            command_buffer.commit();
        }

        #[method(mtkView:drawableSizeWillChange:)]
        #[allow(non_snake_case)]
        unsafe fn mtkView_drawableSizeWillChange(&self, _view: &MTKView, _size: NSSize) {
            // TODO: Handle resize.
        }
    }
);

impl Delegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc();
        let this = this.set_ivars(Ivars {
            start_date: unsafe { NSDate::now() },
            command_queue: OnceCell::default(),
            pipeline_state: OnceCell::default(),
            window: OnceCell::default(),
        });

        unsafe { msg_send_id![super(this), init] }
    }
}

pub struct WindowRunner {
    app: App,
}

impl WindowRunner {
    pub fn new(app: App) -> Result<Box<Self>> {
        Ok(Box::new(WindowRunner { app }))
    }

    pub fn run(&mut self) {
        let mtm = MainThreadMarker::new().unwrap();

        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

        let delegate = Delegate::new(mtm);
        let object = ProtocolObject::from_ref(&*delegate);
        app.setDelegate(Some(object));

        unsafe {
            app.run();
        }
    }
}

pub struct Window {
    gfx: Option<Gfx>,
    file_watcher: FileWatcher,

    pub chars_typed: Vec<char>,
    pub keybinds_typed: Vec<Keybind>,
    pub mousebinds_pressed: Vec<Mousebind>,
    pub mouse_scrolls: Vec<MouseScroll>,
}

impl Window {
    pub fn update<'a>(
        &mut self,
        is_animating: bool,
        ptys: impl Iterator<Item = &'a Pty>,
        files: impl Iterator<Item = &'a Path>,
    ) -> (f32, f32) {
        (0.0, 0.0)
    }

    pub fn is_running(&self) -> bool {
        true
    }

    pub fn is_focused(&self) -> bool {
        true
    }

    pub fn dpi(&self) -> f32 {
        1.0
    }

    pub fn gfx(&mut self) -> &mut Gfx {
        self.gfx.as_mut().unwrap()
    }

    pub fn file_watcher(&self) -> &FileWatcher {
        &self.file_watcher
    }

    pub fn get_char_handler(&self) -> CharHandler {
        CharHandler::new(0)
    }

    pub fn get_keybind_handler(&self) -> KeybindHandler {
        KeybindHandler::new(0)
    }

    pub fn get_mousebind_handler(&self) -> MousebindHandler {
        MousebindHandler::new(0)
    }

    pub fn get_mouse_scroll_handler(&self) -> MouseScrollHandler {
        MouseScrollHandler::new(0)
    }

    pub fn set_clipboard(&mut self, text: &[char], was_copy_implicit: bool) -> Result<()> {
        Ok(())
    }

    pub fn get_clipboard(&mut self) -> Result<&[char]> {
        Ok(&[])
    }

    pub fn was_copy_implicit(&self) -> bool {
        false
    }
}
