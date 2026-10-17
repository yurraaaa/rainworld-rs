//! GPU side of the level graphic: the `level.wgsl` pipeline and the uploaded textures
//! of the current camera.

use std::borrow::Cow;

use wgpu::util::DeviceExt;

use crate::{Camera, LevelGraphics, LevelParams, LevelView};

/// `level.wgsl`'s `Level` uniform.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LevelUniform {
    transform: [f32; 4],
    image_rect: [f32; 4],
    shading: [f32; 4],
    grade: [f32; 4],
    extra: [f32; 4],
}

/// The level pipeline, its bind group layout, the noise sampler and the uniform buffer.
pub(crate) struct LevelPipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    noise_sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
}

/// The current camera's level textures and placement.
pub(crate) struct LevelTextures {
    image: wgpu::TextureView,
    palette: wgpu::TextureView,
    noise: wgpu::TextureView,
    /// Room position and size of the image.
    rect: [f32; 4],
    params: LevelParams,
}

impl LevelTextures {
    /// Room position and size of the image.
    pub(crate) fn rect(&self) -> [f32; 4] {
        self.rect
    }

    /// `_fogAmount` of the palette.
    pub(crate) fn fog_amount(&self) -> f32 {
        self.params.fog_amount
    }

    /// The image, palette and noise views.
    pub(crate) fn views(&self) -> [&wgpu::TextureView; 3] {
        [&self.image, &self.palette, &self.noise]
    }
}

impl LevelPipeline {
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        cache: &wgpu::PipelineCache,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("level"),
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
            label: Some("level.wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/level.wgsl"))),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("level"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("level"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
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
            cache: Some(cache),
        });
        // `_NoiseTex` import settings: bilinear, repeat (mip 0 only; always magnified).
        let noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("noise"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("level uniform"),
            size: std::mem::size_of::<LevelUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout,
            noise_sampler,
            uniform,
        }
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

    /// A 2D texture with one mip holding `data`.
    fn texture_view(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        size: (usize, usize),
        format: wgpu::TextureFormat,
        data: &[u8],
    ) -> wgpu::TextureView {
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.0 as u32,
                height: size.1 as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        device
            .create_texture_with_data(queue, &desc, wgpu::util::TextureDataOrder::LayerMajor, data)
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub(crate) fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    /// Uploads one camera's image, palette and noise texture. Rows go bottom first so
    /// texture coordinates match the game's.
    pub(crate) fn upload(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        level: &LevelGraphics<'_>,
    ) -> LevelTextures {
        let image = level.image;
        let noise_size = rw_assets::NoiseTexture::SIZE;
        LevelTextures {
            image: Self::texture_view(
                device,
                queue,
                "level image",
                (image.width(), image.height()),
                wgpu::TextureFormat::Rgba8Unorm,
                &image.rgba_bottom_up(),
            ),
            palette: Self::texture_view(
                device,
                queue,
                "palette",
                (
                    rw_assets::PaletteTexture::WIDTH,
                    rw_assets::PaletteTexture::HEIGHT,
                ),
                wgpu::TextureFormat::Rgba8Unorm,
                &level.palette.bytes(),
            ),
            noise: Self::texture_view(
                device,
                queue,
                "noise",
                (noise_size, noise_size),
                wgpu::TextureFormat::R8Unorm,
                level.noise.bytes(),
            ),
            rect: [
                level.camera_pos[0],
                level.camera_pos[1],
                image.width() as f32,
                image.height() as f32,
            ],
            params: level.params,
        }
    }

    /// Writes this frame's uniform and binds `textures` with `behind`, the target the
    /// shapes queued before the level were drawn to.
    pub(crate) fn bind(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: &LevelTextures,
        behind: &wgpu::TextureView,
        camera: &Camera,
        view: &LevelView,
    ) -> wgpu::BindGroup {
        let p = &textures.params;
        let uniform = LevelUniform {
            transform: camera.clip_transform(),
            image_rect: textures.rect,
            shading: [view.rain, p.light, p.grime, p.fog_amount],
            grade: [p.darkness, p.contrast, p.saturation, p.hue],
            extra: [p.brightness, p.clouds_speed, p.swarm_room, 0.0],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("level"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&textures.image),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&textures.palette),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&textures.noise),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.noise_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(behind),
                },
            ],
        })
    }
}
