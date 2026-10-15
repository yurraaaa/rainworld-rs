//! GPU side of the room sprites: a `sprite.wgsl` pipeline per game shader, the atlas
//! texture, the room globals, `_NoiseTex2` and the per-frame sprite buffers.

use std::borrow::Cow;
use std::collections::HashMap;

use rw_assets::PngImage;
use rw_graphics::Shader;
use wgpu::util::DeviceExt;

use crate::{Camera, RoomGlobals, RoomVertex, SpriteBatch, SpriteDraw};

/// `sprite.wgsl`'s `Room` uniform.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RoomUniform {
    transform: [f32; 4],
    sprite_rect: [f32; 4],
    screen: [f32; 2],
    rain: f32,
    hologram_threshold: f32,
}

/// The sprite pipelines, the atlas bind group and the frame's buffers.
pub(crate) struct SpritePipeline {
    pipelines: HashMap<Shader, wgpu::RenderPipeline>,
    layout: wgpu::BindGroupLayout,
    noise_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    /// Bound to the atlas set by [`SpritePipeline::set_atlas`].
    bind_group: Option<wgpu::BindGroup>,
    /// `_NoiseTex2`, bound each frame by [`SpritePipeline::upload`].
    noise_group: Option<wgpu::BindGroup>,
    vertices: Option<wgpu::Buffer>,
    indices: Option<wgpu::Buffer>,
}

impl SpritePipeline {
    /// Each shader and its fragment entry point.
    const SHADERS: [(Shader, &'static str); 4] = [
        (Shader::Basic, "fs_basic"),
        (Shader::FlatLight, "fs_flat_light"),
        (Shader::OverseerZip, "fs_overseer_zip"),
        (Shader::Hologram, "fs_hologram"),
    ];

    /// `_hologramThreshold` at render scale 1 (`RainWorld`, decompile 110635: 0.5 when
    /// `Futile.screen.renderScale > 1`).
    const HOLOGRAM_THRESHOLD: f32 = 0.65;

    /// `Blend SrcAlpha OneMinusSrcAlpha` on colour and alpha, as all four shaders.
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
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprites"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });
        let noise_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite noise2"),
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
            label: Some("sprite.wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/sprite.wgsl"))),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprites"),
            bind_group_layouts: &[Some(&layout), Some(&noise_layout)],
            immediate_size: 0,
        });
        let pipelines = Self::SHADERS
            .iter()
            .map(|&(id, entry)| {
                let pipeline = Self::pipeline(device, &pipeline_layout, &shader, format, entry);
                (id, pipeline)
            })
            .collect();
        // Atlas import settings: point filter, clamp, no mips.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite room uniform"),
            size: std::mem::size_of::<RoomUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipelines,
            layout,
            noise_layout,
            sampler,
            uniform,
            bind_group: None,
            noise_group: None,
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
        let attributes = wgpu::vertex_attr_array![
            0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32x4
        ];
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fragment),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RoomVertex>() as u64,
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

    /// Uploads the atlas image as RGBA8 unorm (gamma-space values used as stored),
    /// bottom row first so Futile's texture coordinates address it directly.
    pub(crate) fn set_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &PngImage,
    ) {
        let desc = wgpu::TextureDescriptor {
            label: Some("atlas"),
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
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprites"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        }));
    }

    /// Writes the room globals and the stage camera (`camera` shifted so stage
    /// positions, room minus `cam_pos`, land where their room positions would), and
    /// binds `noise2` (view and sampler) as `_NoiseTex2`.
    pub(crate) fn set_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        cam_pos: [f32; 2],
        globals: &RoomGlobals,
        noise2: (&wgpu::TextureView, &wgpu::Sampler),
    ) {
        let stage = Camera {
            centre: [camera.centre[0] - cam_pos[0], camera.centre[1] - cam_pos[1]],
            ..*camera
        };
        let uniform = RoomUniform {
            transform: stage.clip_transform(),
            sprite_rect: globals.sprite_rect,
            screen: globals.screen_size,
            rain: globals.rain,
            hologram_threshold: Self::HOLOGRAM_THRESHOLD,
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        self.noise_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite noise2"),
            layout: &self.noise_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(noise2.0),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(noise2.1),
                },
            ],
        }));
    }

    /// Writes the frame's sprites.
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        batch: &SpriteBatch,
    ) {
        Self::write(
            device,
            queue,
            &mut self.vertices,
            bytemuck::cast_slice(&batch.vertices),
            wgpu::BufferUsages::VERTEX,
        );
        Self::write(
            device,
            queue,
            &mut self.indices,
            bytemuck::cast_slice(&batch.indices),
            wgpu::BufferUsages::INDEX,
        );
    }

    /// Writes `bytes` to `buffer`, growing it as needed.
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
                label: Some("sprites"),
                size: needed.next_power_of_two().max(4096),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buffer) = buffer {
            queue.write_buffer(buffer, 0, bytes);
        }
    }

    /// Draws one draw of the uploaded batch with its shader.
    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, draw: &SpriteDraw) {
        let (Some(pipeline), Some(bind_group), Some(noise), Some(vertices), Some(indices)) = (
            self.pipelines.get(&draw.shader),
            &self.bind_group,
            &self.noise_group,
            &self.vertices,
            &self.indices,
        ) else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_bind_group(1, noise, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(draw.indices.clone(), 0, 0..1);
    }
}
