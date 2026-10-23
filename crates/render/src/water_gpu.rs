//! GPU side of the game's water: the `water.wgsl` pipelines (`WaterSurface` and
//! `DeepWater`), reading the current camera's level textures and the last grab.
//!
//! Mirrors: -

use std::borrow::Cow;
use std::ops::Range;

use crate::level_gpu::LevelTextures;
use crate::{Camera, LevelView, WaterShader, WaterVertex};

/// `water.wgsl`'s `Water` uniform.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct WaterUniform {
    transform: [f32; 4],
    stage: [f32; 4],
    image: [f32; 4],
    frame: [f32; 4],
}

/// The two water pipelines, their bind group layout, sampler, uniform and vertices.
pub(crate) struct WaterPipeline {
    surface: wgpu::RenderPipeline,
    deep: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    noise_sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    vertices: Option<wgpu::Buffer>,
}

/// This frame's inputs of the water shaders besides the level textures.
pub(crate) struct WaterBinding<'a> {
    pub grab: &'a wgpu::TextureView,
    pub camera: &'a Camera,
    pub view: &'a LevelView,
    /// `_waterDepth`.
    pub depth: f32,
    pub frame_size: (u32, u32),
}

impl WaterPipeline {
    /// `Blend SrcAlpha OneMinusSrcAlpha`, as both water shaders.
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

    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        cache: &wgpu::PipelineCache,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water"),
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
                Self::texture_entry(1, false),
                Self::texture_entry(2, false),
                Self::texture_entry(3, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                Self::texture_entry(5, false),
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("water.wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/water.wgsl"))),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("water"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let surface = Self::pipeline(
            device,
            &pipeline_layout,
            cache,
            &shader,
            format,
            "fs_surface",
        );
        let deep = Self::pipeline(device, &pipeline_layout, cache, &shader, format, "fs_deep");
        let noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water noise"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("water uniform"),
            size: std::mem::size_of::<WaterUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            surface,
            deep,
            layout,
            noise_sampler,
            uniform,
            vertices: None,
        }
    }

    fn pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        cache: &wgpu::PipelineCache,
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
                    array_stride: std::mem::size_of::<WaterVertex>() as u64,
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
            cache: Some(cache),
        })
    }

    fn texture_entry(binding: u32, filterable: bool) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }
    }

    /// Writes the frame's water triangles, growing the buffer as needed.
    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, v: &[WaterVertex]) {
        let bytes: &[u8] = bytemuck::cast_slice(v);
        if bytes.is_empty() {
            return;
        }
        let needed = bytes.len() as u64;
        if self.vertices.as_ref().is_none_or(|b| b.size() < needed) {
            self.vertices = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("water vertices"),
                size: needed.next_power_of_two().max(4096),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buffer) = &self.vertices {
            queue.write_buffer(buffer, 0, bytes);
        }
    }

    /// Writes this frame's uniform and binds the level textures and `binding.grab` (the last `GrabPass`).
    pub(crate) fn bind(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: &LevelTextures,
        binding: &WaterBinding<'_>,
    ) -> wgpu::BindGroup {
        let (view, frame) = (binding.view, binding.frame_size);
        let uniform = WaterUniform {
            transform: binding.camera.clip_transform(),
            stage: [view.origin[0], view.origin[1], view.size[0], view.size[1]],
            image: textures.rect(),
            frame: [frame.0 as f32, frame.1 as f32, view.rain, binding.depth],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        let views = textures.views();
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(views[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.noise_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(binding.grab),
                },
            ],
        })
    }

    /// Draws `vertices` of the uploaded triangles with `shader`.
    pub(crate) fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        bind_group: &wgpu::BindGroup,
        shader: WaterShader,
        vertices: Range<u32>,
    ) {
        let Some(buffer) = &self.vertices else { return };
        pass.set_pipeline(match shader {
            WaterShader::Surface => &self.surface,
            WaterShader::Deep => &self.deep,
        });
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(vertices, 0..1);
    }
}
