//! GPU side of the stage: `stage.wgsl` pipelines (`Futile/Basic`, `Futile/EdgeFade`),
//! one bind group per uploaded texture and the frame's stage buffers.

use std::borrow::Cow;
use std::collections::HashMap;

use rw_assets::PngImage;
use wgpu::util::DeviceExt;

use crate::{
    Camera, SpriteVertex, StageDraw, StageShader, StageTextureId, TextureFilter, TextureSampling,
};

pub(crate) struct StagePipeline {
    basic: wgpu::RenderPipeline,
    edge_fade: wgpu::RenderPipeline,
    texture_layout: wgpu::BindGroupLayout,
    camera: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    textures: HashMap<StageTextureId, wgpu::BindGroup>,
    vertices: Option<wgpu::Buffer>,
    indices: Option<wgpu::Buffer>,
}

impl StagePipeline {
    /// `Blend SrcAlpha OneMinusSrcAlpha`, as both shaders.
    const BLEND: wgpu::BlendState = wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    };

    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stage camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stage texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("stage.wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/stage.wgsl"))),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("stage"),
            bind_group_layouts: &[Some(&camera_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("stage camera"),
            size: std::mem::size_of::<[f32; 4]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stage camera"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        Self {
            basic: Self::pipeline(device, &layout, &shader, format, "fs_basic"),
            edge_fade: Self::pipeline(device, &layout, &shader, format, "fs_edge_fade"),
            texture_layout,
            camera,
            camera_bind_group,
            textures: HashMap::new(),
            vertices: None,
            indices: None,
        }
    }

    fn pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
        fragment: &str,
    ) -> wgpu::RenderPipeline {
        let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fragment),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SpriteVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(fragment),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(Self::BLEND),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    /// Uploads `image` as RGBA8 unorm, bottom row first (Unity's texture coordinates),
    /// replacing any texture with that id.
    pub(crate) fn set_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: StageTextureId,
        image: &PngImage,
        sampling: TextureSampling,
    ) {
        let desc = wgpu::TextureDescriptor {
            label: Some("stage texture"),
            size: wgpu::Extent3d {
                width: image.width() as u32,
                height: image.height() as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let view = device
            .create_texture_with_data(
                queue,
                &desc,
                wgpu::util::TextureDataOrder::LayerMajor,
                &image.rgba_bottom_up(),
            )
            .create_view(&wgpu::TextureViewDescriptor::default());
        let filter = match sampling.filter {
            TextureFilter::Point => wgpu::FilterMode::Nearest,
            TextureFilter::Bilinear => wgpu::FilterMode::Linear,
        };
        let address = if sampling.clamp {
            wgpu::AddressMode::ClampToEdge
        } else {
            wgpu::AddressMode::Repeat
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("stage texture"),
            address_mode_u: address,
            address_mode_v: address,
            mag_filter: filter,
            min_filter: filter,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stage texture"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        self.textures.insert(id, bind_group);
    }

    pub(crate) fn remove_texture(&mut self, id: StageTextureId) {
        self.textures.remove(&id);
    }

    /// Writes the frame's stage quads and the screen camera.
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[SpriteVertex],
        indices: &[u32],
        camera: &Camera,
    ) {
        queue.write_buffer(
            &self.camera,
            0,
            bytemuck::cast_slice(&camera.clip_transform()),
        );
        Self::write(
            device,
            queue,
            &mut self.vertices,
            bytemuck::cast_slice(vertices),
            wgpu::BufferUsages::VERTEX,
        );
        Self::write(
            device,
            queue,
            &mut self.indices,
            bytemuck::cast_slice(indices),
            wgpu::BufferUsages::INDEX,
        );
    }

    fn write(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffer: &mut Option<wgpu::Buffer>,
        bytes: &[u8],
        usage: wgpu::BufferUsages,
    ) {
        if bytes.is_empty() {
            return;
        }
        let needed = bytes.len() as u64;
        if buffer.as_ref().is_none_or(|b| b.size() < needed) {
            *buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("stage"),
                size: needed.next_power_of_two().max(4096),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buffer) = buffer {
            queue.write_buffer(buffer, 0, bytes);
        }
    }

    /// Draws one stage draw; a draw whose texture is not uploaded is skipped.
    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, draw: &StageDraw) {
        let (Some(texture), Some(vertices), Some(indices)) = (
            self.textures.get(&draw.texture),
            &self.vertices,
            &self.indices,
        ) else {
            return;
        };
        pass.set_pipeline(match draw.shader {
            StageShader::Basic => &self.basic,
            StageShader::EdgeFade => &self.edge_fade,
        });
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_bind_group(1, texture, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(draw.indices.clone(), 0, 0..1);
    }
}
