use core::str;
use std::{
    mem::offset_of,
    ptr::{copy_nonoverlapping, null_mut},
    slice::from_raw_parts,
};

use windows::{
    core::{s, Interface, Result},
    Win32::{
        Foundation::{HMODULE, HWND, RECT, TRUE},
        Graphics::{
            Direct3D::{
                Fxc::{D3DCompile, D3DCOMPILE_DEBUG},
                *,
            },
            Direct3D11::*,
            DirectComposition::*,
            Dxgi::{Common::*, *},
        },
        UI::WindowsAndMessaging::GetClientRect,
    },
};

use crate::{
    geometry::{matrix::ortho, quad::Quad, rect::Rect},
    platform::{
        aliases::AnyText,
        gfx::SpriteKind,
        text_cache::{AtlasDimensions, GlyphCacheResult, GlyphSpan, GlyphSpans},
    },
    ui::color::Color,
};

use super::text::Text;

const SHADER_CODE: &str = r#"
cbuffer constants : register(b0) {
	float4x4 projectionMatrix;
	float2 textureSize;
}

struct VSInput {
	float2 position : POSITION;
	float3 uv : TEX;
	float4 color : COLOR0;
};

struct VSOutput {
	float4 position : SV_POSITION;
	float3 uv : TEX;
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
	output.uv = float3(input.uv.xy / textureSize.xy, input.uv.z);
	output.color = input.color;
	return output;
}

PSOutput PsMain(VSOutput input) : SV_Target {
    float4 normalizedColor = input.color / 255.0;
    float4 textureSample = _texture.Sample(_sampler, input.uv.xy);

    float4 alphaMasks[] = {
        textureSample.rgbr,
        textureSample.aaaa,
        normalizedColor.aaaa,
    };

    PSOutput output;
    output.color = input.uv.z == 1.0 ? float4(textureSample.rgb / textureSample.a, 1.0) : float4(normalizedColor.rgb, 1.0);
    output.alphaMask = alphaMasks[(int)input.uv.z];
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
    texture_size: [f32; 2],
}

#[derive(Clone, Copy)]
#[allow(unused)]
struct Vertex {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
    kind: f32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

const SAMPLE_COUNT: u32 = 4;
const PIXEL_FORMAT: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;

pub struct Gfx {
    device: ID3D11Device,
    _composition_device: IDCompositionDevice,
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
    context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain1,
    rasterizer_state: ID3D11RasterizerState,
    blend_state: ID3D11BlendState,
    vertex_shader: ID3D11VertexShader,
    input_layout: ID3D11InputLayout,
    pixel_shader: ID3D11PixelShader,
    texture_data: Option<TextureData>,
    uniform_buffer: ID3D11Buffer,

    msaa_color_texture: Option<ID3D11Texture2D>,
    render_target_view: Option<ID3D11RenderTargetView>,
    width: i32,
    height: i32,
    scale: f32,
    bounds: Rect,

    vertices: Vec<Vertex>,
    vertex_buffer: Option<ID3D11Buffer>,
    vertex_buffer_capacity: usize,

    indices: Vec<u32>,
    index_buffer: Option<ID3D11Buffer>,
    index_buffer_capacity: usize,

    text: Option<AnyText>,
    glyph_cache_result: GlyphCacheResult,
}

impl Gfx {
    pub unsafe fn new(scale: f32, hwnd: HWND) -> Result<Self> {
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
                HMODULE(null_mut()),
                flags,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device_result),
                None,
                Some(&mut context_result),
            )?;

            (device_result.unwrap(), context_result.unwrap())
        };

        let dxgi_device: IDXGIDevice4 = device.cast()?;
        let composition_device: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;
        let target = composition_device.CreateTargetForHwnd(hwnd, true)?;
        let visual = composition_device.CreateVisual()?;

        let swap_chain = {
            let factory: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS::default())?;

            let mut rect = RECT::default();
            GetClientRect(hwnd, &mut rect).unwrap();

            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Format: PIXEL_FORMAT,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    ..Default::default()
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                Width: (rect.right - rect.left) as u32,
                Height: (rect.bottom - rect.top) as u32,
                ..Default::default()
            };

            factory
                .CreateSwapChainForComposition(&device, &desc, None)
                .unwrap()
        };

        visual.SetContent(&swap_chain)?;
        target.SetRoot(&visual)?;
        composition_device.Commit()?;

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
                    Format: DXGI_FORMAT_R32G32B32_FLOAT,
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
                ByteWidth: ((size_of::<Uniform>() + 0xF) & 0xFFFFFFF0) as u32,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                ..Default::default()
            };

            let mut uniform_buffer_result = None;

            device.CreateBuffer(&desc, None, Some(&mut uniform_buffer_result))?;

            uniform_buffer_result.unwrap()
        };

        let gfx = Self {
            device,
            _composition_device: composition_device,
            _target: target,
            _visual: visual,
            context,
            swap_chain,
            rasterizer_state,
            blend_state,
            vertex_shader,
            input_layout,
            pixel_shader,
            texture_data: None,
            uniform_buffer,

            msaa_color_texture: None,
            render_target_view: None,
            width: 0,
            height: 0,
            scale,
            bounds: Rect::ZERO,

            vertices: Vec::new(),
            vertex_buffer: None,
            vertex_buffer_capacity: 0,

            indices: Vec::new(),
            index_buffer: None,
            index_buffer_capacity: 0,

            text: None,
            glyph_cache_result: GlyphCacheResult::Hit,
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
                Format: PIXEL_FORMAT,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    ..Default::default()
                },
                Usage: D3D11_USAGE_DEFAULT,
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
        self.msaa_color_texture = None;

        let quality = self
            .device
            .CheckMultisampleQualityLevels(PIXEL_FORMAT, SAMPLE_COUNT)?;

        let desc = D3D11_TEXTURE2D_DESC {
            Width: width as u32,
            Height: height as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: PIXEL_FORMAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: SAMPLE_COUNT,
                Quality: quality - 1,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            ..Default::default()
        };

        self.device
            .CreateTexture2D(&desc, None, Some(&mut self.msaa_color_texture))?;
        let msaa_color_texture = self.msaa_color_texture.as_ref().unwrap();

        self.device.CreateRenderTargetView(
            msaa_color_texture,
            None,
            Some(&mut self.render_target_view),
        )?;

        self.swap_chain
            .ResizeBuffers(
                0,
                self.width as u32,
                self.height as u32,
                DXGI_FORMAT_UNKNOWN,
                DXGI_SWAP_CHAIN_FLAG::default(),
            )
            .unwrap();

        let viewport = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: self.width as f32,
            Height: self.height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };

        self.context.RSSetViewports(Some(&[viewport]));

        Ok(())
    }

    unsafe fn update_uniform(&mut self, uniform: Uniform) {
        let mut resource = D3D11_MAPPED_SUBRESOURCE::default();

        self.context
            .Map(
                &self.uniform_buffer,
                0,
                D3D11_MAP_WRITE_DISCARD,
                0,
                Some(&mut resource),
            )
            .unwrap();

        copy_nonoverlapping(&uniform, resource.pData as *mut Uniform, 1);

        self.context.Unmap(&self.uniform_buffer, 0);
    }

    pub fn set_font(&mut self, font_name: &str, font_size: f32, scale: f32) {
        self.scale = scale;

        self.text = AnyText::new(font_name, |font_name| unsafe {
            Text::new(font_name, font_size, scale, &self.device)
        })
        .ok();
    }

    pub fn glyph_spans(&mut self, text: &str) -> GlyphSpans {
        let Some(platform_text) = &mut self.text else {
            return Default::default();
        };

        let (spans, result) = platform_text.glyph_spans(text);
        self.glyph_cache_result = self.glyph_cache_result.worse(result);

        spans
    }

    pub fn glyph_span(&mut self, index: usize) -> GlyphSpan {
        self.text
            .as_mut()
            .map(|text| text.glyph_span(index))
            .unwrap_or_default()
    }

    fn handle_glyph_cache_result(&mut self) -> Option<()> {
        let atlas = &self.text.as_ref()?.cache.atlas;

        match self.glyph_cache_result {
            GlyphCacheResult::Hit => {}
            GlyphCacheResult::Miss => unsafe {
                let texture_data = self.texture_data.as_ref()?;

                self.context.UpdateSubresource(
                    &texture_data.texture,
                    0,
                    None,
                    atlas.data.as_ptr() as _,
                    atlas.dimensions.width as u32 * 4,
                    (atlas.dimensions.width * atlas.dimensions.height) as u32 * 4,
                );
            },
            GlyphCacheResult::Resize => unsafe {
                self.texture_data = Some(
                    Self::create_texture(
                        &self.device,
                        atlas.dimensions.width as u32,
                        atlas.dimensions.height as u32,
                        &atlas.data,
                    )
                    .ok()?,
                );
            },
        }

        self.glyph_cache_result = GlyphCacheResult::Hit;

        Some(())
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

        if let Some(text) = &mut self.text {
            text.swap_caches();
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
        self.handle_glyph_cache_result();

        let atlas_dimensions = self.atlas_dimensions();

        let uniform = Uniform {
            projection_matrix: ortho(0.0, self.width as f32, 0.0, self.height as f32, -1.0, 1.0),
            texture_size: [
                atlas_dimensions.width as f32,
                atlas_dimensions.height as f32,
            ],
        };

        unsafe {
            self.update_uniform(uniform);

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

                self.vertex_buffer.take();

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

                self.index_buffer.take();

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

            if let Some(texture_data) = self.texture_data.as_ref() {
                self.context
                    .PSSetShaderResources(0, Some(&[Some(texture_data.texture_view.clone())]));
                self.context
                    .PSSetSamplers(0, Some(&[Some(texture_data.sampler_state.clone())]));
            }

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

            let backbuffer: ID3D11Texture2D = self.swap_chain.GetBuffer(0).unwrap();

            self.context.ResolveSubresource(
                &backbuffer,
                0,
                self.msaa_color_texture.as_ref().unwrap(),
                0,
                PIXEL_FORMAT,
            );
        }
    }

    pub fn add_sprite(&mut self, src: Rect, dst: Quad, color: Color, kind: SpriteKind) {
        let dst = dst.offset_by(self.bounds);

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

        let kind = kind as usize as f32;

        self.vertices.extend_from_slice(&[
            Vertex {
                x: dst.top_left.x,
                y: dst.top_left.y,
                u: uv_left,
                v: uv_top,
                kind,
                r,
                g,
                b,
                a,
            },
            Vertex {
                x: dst.top_right.x,
                y: dst.top_right.y,
                u: uv_right,
                v: uv_top,
                kind,
                r,
                g,
                b,
                a,
            },
            Vertex {
                x: dst.bottom_right.x,
                y: dst.bottom_right.y,
                u: uv_right,
                v: uv_bottom,
                kind,
                r,
                g,
                b,
                a,
            },
            Vertex {
                x: dst.bottom_left.x,
                y: dst.bottom_left.y,
                u: uv_left,
                v: uv_bottom,
                kind,
                r,
                g,
                b,
                a,
            },
        ]);
    }

    pub fn atlas_dimensions(&self) -> &AtlasDimensions {
        self.text
            .as_ref()
            .map(|text| &text.cache.atlas.dimensions)
            .unwrap_or(&AtlasDimensions::ZERO)
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn width(&self) -> f32 {
        self.width as f32
    }

    pub fn height(&self) -> f32 {
        self.height as f32
    }
}
