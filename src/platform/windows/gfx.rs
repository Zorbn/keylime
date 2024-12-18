use core::str;
use std::{borrow::Borrow, mem::offset_of, ptr::copy_nonoverlapping, slice::from_raw_parts};

use windows::{
    core::{s, Result},
    Win32::{
        Foundation::{RECT, TRUE},
        Graphics::{
            Direct3D::{
                Fxc::{D3DCompile, D3DCOMPILE_DEBUG},
                *,
            },
            Direct3D11::*,
            Dxgi::{Common::*, *},
        },
    },
};

use crate::{
    geometry::{
        matrix::ortho,
        rect::Rect,
        side::{SIDE_BOTTOM, SIDE_LEFT, SIDE_RIGHT, SIDE_TOP},
    },
    ui::color::Color,
};

use super::{
    text::{AtlasDimensions, Text},
    window::Window,
};

const SHADER_CODE: &str = r#"
cbuffer constants : register(b0) {
	float4x4 projectionMatrix;
}

struct VSInput {
	float2 position : POSITION;
	float2 uv : TEX;
	float4 color : COLOR0;
};

struct VSOutput {
	float4 position : SV_POSITION;
	float2 uv : TEX;
	float4 color : COLOR0;
};

struct PSOutput {
    float4 color : SV_Target0;
    float4 alphaMask : SV_Target1;
};

Texture2D _texture : register(t0);
SamplerState _sampler : register(s0);

VSOutput VsMain(VSInput input) {
	VSOutput output;
	output.position = mul(projectionMatrix, float4(input.position.xy, 0.0f, 1.0f));
	output.uv = input.uv;
	output.color = input.color;
	return output;
}

PSOutput PsMain(VSOutput input) : SV_Target {
    float4 normalizedColor = input.color / 255.0;

    PSOutput output;
    output.color = float4(normalizedColor.rgb, 1.0);
    output.alphaMask = input.uv.y < 0.0 ?
        normalizedColor.aaaa :
        _texture.Sample(_sampler, input.uv).rgbr;
    return output;
}
"#;

struct TextureData {
    #[allow(unused)]
    texture: ID3D11Texture2D,
    texture_view: ID3D11ShaderResourceView,
    sampler_state: ID3D11SamplerState,
}

#[allow(unused)]
struct Uniform {
    projection_matrix: [f32; 16],
}

#[derive(Clone, Copy)]
#[allow(unused)]
struct Vertex {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

const TAB_WIDTH: usize = 4;

pub struct Gfx {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain1,
    rasterizer_state: ID3D11RasterizerState,
    blend_state: ID3D11BlendState,
    vertex_shader: ID3D11VertexShader,
    input_layout: ID3D11InputLayout,
    pixel_shader: ID3D11PixelShader,
    texture_data: TextureData,
    uniform_buffer: ID3D11Buffer,

    render_target_view: Option<ID3D11RenderTargetView>,
    width: i32,
    height: i32,
    bounds: Rect,

    vertices: Vec<Vertex>,
    vertex_buffer: Option<ID3D11Buffer>,
    vertex_buffer_capacity: usize,

    indices: Vec<u32>,
    index_buffer: Option<ID3D11Buffer>,
    index_buffer_capacity: usize,

    atlas_dimensions: AtlasDimensions,
}

impl Gfx {
    pub unsafe fn new(font_name: &str, font_size: f32, window: &Window) -> Result<Self> {
        let (device, context) = {
            let mut device_result = None;
            let mut context_result = None;

            let flags = if cfg!(debug_assertions) {
                D3D11_CREATE_DEVICE_DEBUG | D3D11_CREATE_DEVICE_BGRA_SUPPORT
            } else {
                D3D11_CREATE_DEVICE_BGRA_SUPPORT
            };

            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                flags,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device_result),
                None,
                Some(&mut context_result),
            )?;

            (device_result.unwrap(), context_result.unwrap())
        };

        let swap_chain = {
            let factory: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS::default())?;

            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    ..Default::default()
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_NONE,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                ..Default::default()
            };

            factory.CreateSwapChainForHwnd(&device, window.hwnd(), &desc, None, None)?
        };

        let rasterizer_state = {
            let desc = D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_BACK,
                DepthClipEnable: TRUE,
                ScissorEnable: TRUE,
                ..Default::default()
            };

            let mut rasterizer_state_result = None;

            device.CreateRasterizerState(&desc, Some(&mut rasterizer_state_result))?;

            rasterizer_state_result.unwrap()
        };

        let blend_state = {
            let mut desc = D3D11_BLEND_DESC {
                ..Default::default()
            };

            desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: TRUE,
                SrcBlend: D3D11_BLEND_SRC1_COLOR,
                DestBlend: D3D11_BLEND_INV_SRC1_COLOR,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_ZERO,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            };

            let mut blend_state_result = None;

            device.CreateBlendState(&desc, Some(&mut blend_state_result))?;

            blend_state_result.unwrap()
        };

        let compile_flags = if cfg!(debug_assertions) {
            D3DCOMPILE_DEBUG
        } else {
            0
        };

        let (vertex_shader, input_layout) = {
            let mut compiled_code = None;
            let mut compile_error = None;

            if D3DCompile(
                SHADER_CODE.as_ptr() as _,
                SHADER_CODE.len(),
                None,
                None,
                None,
                s!("VsMain"),
                s!("vs_5_0"),
                compile_flags,
                0,
                &mut compiled_code,
                Some(&mut compile_error),
            )
            .is_err()
            {
                Self::handle_shader_error(compile_error.unwrap());
            }

            let compiled_code = compiled_code.unwrap();
            let compiled_code_slice = from_raw_parts(
                compiled_code.GetBufferPointer() as *const u8,
                compiled_code.GetBufferSize(),
            );

            let mut vertex_shader_result = None;

            device.CreateVertexShader(
                compiled_code_slice,
                None,
                Some(&mut vertex_shader_result),
            )?;

            let descs = [
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: s!("POSITION"),
                    SemanticIndex: 0,
                    Format: DXGI_FORMAT_R32G32_FLOAT,
                    InputSlot: 0,
                    AlignedByteOffset: offset_of!(Vertex, x) as u32,
                    InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                    InstanceDataStepRate: 0,
                },
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: s!("TEX"),
                    SemanticIndex: 0,
                    Format: DXGI_FORMAT_R32G32_FLOAT,
                    InputSlot: 0,
                    AlignedByteOffset: offset_of!(Vertex, u) as u32,
                    InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                    InstanceDataStepRate: 0,
                },
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: s!("COLOR"),
                    SemanticIndex: 0,
                    Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
                    InputSlot: 0,
                    AlignedByteOffset: offset_of!(Vertex, r) as u32,
                    InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                    InstanceDataStepRate: 0,
                },
            ];

            let mut input_layout_result = None;

            device.CreateInputLayout(
                &descs,
                compiled_code_slice,
                Some(&mut input_layout_result),
            )?;

            (vertex_shader_result.unwrap(), input_layout_result.unwrap())
        };

        let pixel_shader = {
            let mut compiled_code = None;
            let mut compile_error = None;

            if D3DCompile(
                SHADER_CODE.as_ptr() as _,
                SHADER_CODE.len(),
                None,
                None,
                None,
                s!("PsMain"),
                s!("ps_5_0"),
                compile_flags,
                0,
                &mut compiled_code,
                Some(&mut compile_error),
            )
            .is_err()
            {
                Self::handle_shader_error(compile_error.unwrap());
            }

            let compiled_code = compiled_code.unwrap();
            let compiled_code_slice = from_raw_parts(
                compiled_code.GetBufferPointer() as *const u8,
                compiled_code.GetBufferSize(),
            );

            let mut pixel_shader_result = None;

            device.CreatePixelShader(compiled_code_slice, None, Some(&mut pixel_shader_result))?;

            pixel_shader_result.unwrap()
        };

        let uniform_buffer = {
            let desc = D3D11_BUFFER_DESC {
                ByteWidth: ((size_of::<Uniform>() + 0xf) & 0xfffffff0) as u32,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                ..Default::default()
            };

            let mut uniform_buffer_result = None;

            device.CreateBuffer(&desc, None, Some(&mut uniform_buffer_result))?;

            uniform_buffer_result.unwrap()
        };

        let (texture_data, atlas_dimensions) =
            Self::create_atlas_texture(&device, font_name, font_size, window.scale())?;

        let gfx = Self {
            device,
            context,
            swap_chain,
            rasterizer_state,
            blend_state,
            vertex_shader,
            input_layout,
            pixel_shader,
            texture_data,
            uniform_buffer,

            render_target_view: None,
            width: 0,
            height: 0,
            bounds: Rect::zero(),

            vertices: Vec::new(),
            vertex_buffer: None,
            vertex_buffer_capacity: 0,

            indices: Vec::new(),
            index_buffer: None,
            index_buffer_capacity: 0,

            atlas_dimensions,
        };

        Ok(gfx)
    }

    unsafe fn handle_shader_error(compile_error: ID3DBlob) {
        let message = str::from_utf8_unchecked(from_raw_parts(
            compile_error.GetBufferPointer() as _,
            compile_error.GetBufferSize(),
        ));

        panic!("Shader compile error: {}", message);
    }

    unsafe fn create_atlas_texture(
        device: &ID3D11Device,
        font_name: &str,
        font_size: f32,
        scale: f32,
    ) -> Result<(TextureData, AtlasDimensions)> {
        let mut text = Text::new(font_name, font_size, scale)?;
        let atlas = text.generate_atlas().unwrap();

        let texture_data = Self::create_texture(
            device,
            atlas.dimensions.width as u32,
            atlas.dimensions.height as u32,
            &atlas.data,
        )?;

        Ok((texture_data, atlas.dimensions))
    }

    unsafe fn create_texture(
        device: &ID3D11Device,
        width: u32,
        height: u32,
        atlas_data: &[u8],
    ) -> Result<TextureData> {
        let (texture, texture_view, sampler_state) = {
            let sampler_desc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                ComparisonFunc: D3D11_COMPARISON_NEVER,
                ..Default::default()
            };

            let mut sampler_state_result = None;

            device.CreateSamplerState(&sampler_desc, Some(&mut sampler_state_result))?;

            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    ..Default::default()
                },
                Usage: D3D11_USAGE_IMMUTABLE,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                ..Default::default()
            };

            let init_data = D3D11_SUBRESOURCE_DATA {
                pSysMem: atlas_data.as_ptr() as _,
                SysMemPitch: width * 4,
                ..Default::default()
            };

            let mut texture_result = None;

            device.CreateTexture2D(&desc, Some(&init_data), Some(&mut texture_result))?;

            let texture = texture_result.unwrap();

            let view_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
                Format: desc.Format,
                ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                    Texture2D: D3D11_TEX2D_SRV {
                        MostDetailedMip: 0,
                        MipLevels: 1,
                    },
                },
            };

            let mut texture_view_result = None;

            device.CreateShaderResourceView(
                &texture,
                Some(&view_desc),
                Some(&mut texture_view_result),
            )?;

            (
                texture,
                texture_view_result.unwrap(),
                sampler_state_result.unwrap(),
            )
        };

        Ok(TextureData {
            texture,
            texture_view,
            sampler_state,
        })
    }

    pub unsafe fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }

        self.width = width;
        self.height = height;

        self.context.OMSetRenderTargets(None, None);
        self.render_target_view = None;

        self.swap_chain.ResizeBuffers(
            0,
            self.width as u32,
            self.height as u32,
            DXGI_FORMAT_UNKNOWN,
            DXGI_SWAP_CHAIN_FLAG::default(),
        )?;

        let backbuffer: ID3D11Texture2D = self.swap_chain.GetBuffer(0).unwrap();

        self.device.CreateRenderTargetView(
            &backbuffer,
            None,
            Some(&mut self.render_target_view),
        )?;

        let viewport = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: self.width as f32,
            Height: self.height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };

        self.context.RSSetViewports(Some(&[viewport]));

        let uniform = Uniform {
            projection_matrix: ortho(0.0, self.width as f32, 0.0, self.height as f32, -1.0, 1.0),
        };

        let mut resource = D3D11_MAPPED_SUBRESOURCE::default();
        self.context.Map(
            &self.uniform_buffer,
            0,
            D3D11_MAP_WRITE_DISCARD,
            0,
            Some(&mut resource),
        )?;

        copy_nonoverlapping(&uniform, resource.pData as *mut Uniform, 1);

        self.context.Unmap(&self.uniform_buffer, 0);

        Ok(())
    }

    pub fn update_font(&mut self, font_name: &str, font_size: f32, scale: f32) {
        unsafe {
            if let Ok(atlas_texture) =
                Self::create_atlas_texture(&self.device, font_name, font_size, scale)
            {
                (self.texture_data, self.atlas_dimensions) = atlas_texture;
            }
        }
    }

    pub fn begin_frame(&mut self, clear_color: Color) {
        let render_target_view = self.render_target_view.as_ref().unwrap();

        unsafe {
            self.context.ClearRenderTargetView(
                render_target_view,
                &[
                    clear_color.r as f32 / 255.0,
                    clear_color.g as f32 / 255.0,
                    clear_color.b as f32 / 255.0,
                    clear_color.a as f32 / 255.0,
                ],
            );
        }
    }

    pub fn end_frame(&mut self) {
        unsafe {
            self.swap_chain.Present(1, DXGI_PRESENT::default()).unwrap();
        }
    }

    pub fn begin(&mut self, bounds: Option<Rect>) {
        self.vertices.clear();
        self.indices.clear();

        if let Some(bounds) = bounds {
            self.bounds = bounds;
        } else {
            self.bounds = Rect::new(0.0, 0.0, self.width as f32, self.height as f32);
        }
    }

    pub fn end(&mut self) {
        unsafe {
            if (self.vertex_buffer.is_none() && !self.vertices.is_empty())
                || self.vertex_buffer_capacity < self.vertices.len()
            {
                let desc = D3D11_BUFFER_DESC {
                    ByteWidth: (size_of::<Vertex>() * self.vertices.capacity()) as u32,
                    Usage: D3D11_USAGE_DYNAMIC,
                    BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
                    CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                    ..Default::default()
                };

                self.device
                    .CreateBuffer(&desc, None, Some(&mut self.vertex_buffer))
                    .unwrap();

                self.vertex_buffer_capacity = self.vertices.capacity();
            }

            if (self.index_buffer.is_none() && !self.indices.is_empty())
                || self.index_buffer_capacity < self.indices.len()
            {
                let desc = D3D11_BUFFER_DESC {
                    ByteWidth: (size_of::<u32>() * self.indices.capacity()) as u32,
                    Usage: D3D11_USAGE_DYNAMIC,
                    BindFlags: D3D11_BIND_INDEX_BUFFER.0 as u32,
                    CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                    ..Default::default()
                };

                self.device
                    .CreateBuffer(&desc, None, Some(&mut self.index_buffer))
                    .unwrap();

                self.index_buffer_capacity = self.indices.capacity();
            }

            let Some(vertex_buffer) = &self.vertex_buffer else {
                return;
            };

            let Some(index_buffer) = &self.index_buffer else {
                return;
            };

            let mut resource = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(
                    vertex_buffer,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut resource),
                )
                .unwrap();

            copy_nonoverlapping(
                self.vertices.as_ptr(),
                resource.pData as *mut Vertex,
                self.vertices.len(),
            );

            self.context.Unmap(vertex_buffer, 0);

            let mut resource = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(
                    index_buffer,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut resource),
                )
                .unwrap();

            copy_nonoverlapping(
                self.indices.as_ptr(),
                resource.pData as *mut u32,
                self.indices.len(),
            );

            self.context.Unmap(index_buffer, 0);

            self.context.RSSetScissorRects(Some(&[RECT {
                left: self.bounds.x as i32,
                right: (self.bounds.x + self.bounds.width) as i32,
                top: self.bounds.y as i32,
                bottom: (self.bounds.y + self.bounds.height) as i32,
            }]));

            self.context
                .OMSetRenderTargets(Some(&[self.render_target_view.clone()]), None);

            self.context.IASetInputLayout(&self.input_layout);

            self.context
                .PSSetShaderResources(0, Some(&[Some(self.texture_data.texture_view.clone())]));
            self.context
                .PSSetSamplers(0, Some(&[Some(self.texture_data.sampler_state.clone())]));

            self.context.IASetVertexBuffers(
                0,
                1,
                Some(&self.vertex_buffer),
                Some(&(size_of::<Vertex>() as u32)),
                Some(&0),
            );

            self.context
                .IASetIndexBuffer(Some(index_buffer), DXGI_FORMAT_R32_UINT, 0);

            self.context
                .IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

            self.context.VSSetShader(&self.vertex_shader, None);
            self.context.PSSetShader(&self.pixel_shader, None);
            self.context
                .VSSetConstantBuffers(0, Some(&[Some(self.uniform_buffer.clone())]));

            self.context.RSSetState(&self.rasterizer_state);
            self.context
                .OMSetBlendState(&self.blend_state, None, 0xFFFFFFFFu32);

            self.context.DrawIndexed(self.indices.len() as u32, 0, 0);
        }
    }

    pub fn add_sprite(&mut self, src: Rect, dst: Rect, color: Color) {
        let left = dst.x + self.bounds.x;
        let top = dst.y + self.bounds.y;
        let right = left + dst.width;
        let bottom = top + dst.height;

        let uv_left = src.x;
        let uv_right = src.x + src.width;
        let uv_top = src.y;
        let uv_bottom = src.y + src.height;

        let r = color.r as f32;
        let g = color.g as f32;
        let b = color.b as f32;
        let a = color.a as f32;

        let vertices_len = self.vertices.len() as u32;

        self.indices.extend_from_slice(&[
            vertices_len,
            vertices_len + 1,
            vertices_len + 2,
            vertices_len,
            vertices_len + 2,
            vertices_len + 3,
        ]);

        self.vertices.extend_from_slice(&[
            Vertex {
                x: left,
                y: top,
                u: uv_left,
                v: uv_top,
                r,
                g,
                b,
                a,
            },
            Vertex {
                x: right,
                y: top,
                u: uv_right,
                v: uv_top,
                r,
                g,
                b,
                a,
            },
            Vertex {
                x: right,
                y: bottom,
                u: uv_right,
                v: uv_bottom,
                r,
                g,
                b,
                a,
            },
            Vertex {
                x: left,
                y: bottom,
                u: uv_left,
                v: uv_bottom,
                r,
                g,
                b,
                a,
            },
        ]);
    }

    pub fn measure_text(text: impl IntoIterator<Item = impl Borrow<char>>) -> isize {
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
        let mut current_visual_x = 0isize;
        let mut x = 0isize;

        for c in text.into_iter() {
            let c = *c.borrow();

            current_visual_x += if c == '\t' { TAB_WIDTH as isize } else { 1 };

            if current_visual_x > visual_x {
                return x;
            }

            x += 1;
        }

        x
    }

    pub fn get_char_width(c: char) -> isize {
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
        let min_char = b' ' as u32;
        let max_char = b'~' as u32;

        let AtlasDimensions {
            width,
            glyph_offset_x,
            glyph_step_x,
            glyph_width,
            glyph_height,
            ..
        } = self.atlas_dimensions;

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
        self.add_sprite(Rect::new(-1.0, -1.0, -1.0, -1.0), rect, color);
    }

    pub fn glyph_width(&self) -> f32 {
        self.atlas_dimensions.glyph_width
    }

    pub fn glyph_height(&self) -> f32 {
        self.atlas_dimensions.glyph_height
    }

    pub fn line_height(&self) -> f32 {
        self.atlas_dimensions.line_height
    }

    pub fn line_padding(&self) -> f32 {
        (self.line_height() - self.glyph_height()) / 2.0
    }

    pub fn border_width(&self) -> f32 {
        1.0
    }

    pub fn width(&self) -> f32 {
        self.width as f32
    }

    pub fn height(&self) -> f32 {
        self.height as f32
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
