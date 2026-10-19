//! Draws an [`Overlay`] over a target (the window's surface texture, or the frame when
//! headless).

use std::borrow::Cow;

use wgpu::util::DeviceExt;

use crate::{Overlay, OverlayVertex};

pub(crate) struct OverlayPipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    /// The uploaded overlay: buffers and draws.
    uploaded: Option<Uploaded>,
}

struct Uploaded {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    draws: Vec<crate::OverlayDraw>,
}

impl OverlayPipeline {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        cache: &wgpu::PipelineCache,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("overlay"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
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
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("overlay.wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../shaders/overlay.wgsl"
            ))),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("overlay"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overlay"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<OverlayVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: Some(cache),
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("overlay target"),
            size: std::mem::size_of::<[f32; 4]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("overlay"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let white = Self::texture_view(device, queue, 1, 1, &[255; 4]);
        let bind_group = Self::bind(device, &layout, &uniform, &white, &sampler);
        Self {
            pipeline,
            layout,
            uniform,
            sampler,
            bind_group,
            uploaded: None,
        }
    }

    fn texture_view(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> wgpu::TextureView {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("overlay atlas"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            rgba,
        );
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn bind(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("overlay"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Replaces the atlas (`width * height` RGBA pixels, rows top to bottom).
    pub(crate) fn set_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) {
        if width == 0 || height == 0 || rgba.len() != (width * height * 4) as usize {
            log::warn!("overlay texture {width}x{height} with {} bytes", rgba.len());
            return;
        }
        let view = Self::texture_view(device, queue, width, height, rgba);
        self.bind_group = Self::bind(device, &self.layout, &self.uniform, &view, &self.sampler);
    }

    /// Uploads `overlay` for a target of `size`; an empty one clears it.
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        overlay: &Overlay,
        size: (u32, u32),
    ) {
        if overlay.is_empty() || overlay.vertices.is_empty() || overlay.indices.is_empty() {
            self.uploaded = None;
            return;
        }
        let target = [size.0 as f32, size.1 as f32, 0.0, 0.0];
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&target));
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("overlay vertices"),
            contents: bytemuck::cast_slice(&overlay.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("overlay indices"),
            contents: bytemuck::cast_slice(&overlay.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.uploaded = Some(Uploaded {
            vertices,
            indices,
            draws: overlay.draws.clone(),
        });
    }

    /// Draws the uploaded overlay over `target` (of `size`), keeping what is there.
    pub(crate) fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        let Some(up) = &self.uploaded else { return };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("overlay pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, up.vertices.slice(..));
        pass.set_index_buffer(up.indices.slice(..), wgpu::IndexFormat::Uint32);
        for draw in &up.draws {
            let Some((x, y, w, h)) = draw.scissor(size) else {
                continue;
            };
            pass.set_scissor_rect(x, y, w, h);
            pass.draw_indexed(draw.first..draw.first + draw.count, 0, 0..1);
        }
    }
}
