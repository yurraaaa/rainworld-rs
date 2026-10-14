//! Draws the offscreen frame into the window's letterbox rectangle ([`PresentRect`]).

use std::borrow::Cow;

use crate::{PresentFilter, PresentRect};

pub(crate) struct PresentPipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    /// Nearest, then linear sampling.
    samplers: [wgpu::Sampler; 2],
    /// The frame texture with each sampler, once [`PresentPipeline::bind`] was called.
    bind_groups: Option<[wgpu::BindGroup; 2]>,
}

impl PresentPipeline {
    /// A pipeline writing `format`.
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present"),
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
            label: Some("present.wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../shaders/present.wgsl"
            ))),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
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
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let samplers = [wgpu::FilterMode::Nearest, wgpu::FilterMode::Linear].map(|filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("present"),
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        });
        Self {
            pipeline,
            layout,
            samplers,
            bind_groups: None,
        }
    }

    /// Samples `frame` from now on.
    pub(crate) fn bind(&mut self, device: &wgpu::Device, frame: &wgpu::TextureView) {
        self.bind_groups = Some(self.samplers.each_ref().map(|sampler| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("present"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(frame),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        }));
    }

    /// Clears `target` to black and draws the bound frame into `rect`.
    pub(crate) fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        rect: PresentRect,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(
            rect.x as f32,
            rect.y as f32,
            rect.width as f32,
            rect.height as f32,
            0.0,
            1.0,
        );
        let Some(bind_groups) = &self.bind_groups else {
            return;
        };
        let bind_group = match rect.filter {
            PresentFilter::Nearest => &bind_groups[0],
            PresentFilter::Linear => &bind_groups[1],
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
