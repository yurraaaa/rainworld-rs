//! GPU side of the level graphic: the `level.wgsl` pipeline and the uploaded textures
//! of the current camera.
//!
//! Mirrors: - (port-only: wgpu side of the level graphic)

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
    light_dir: [f32; 4],
    atmosphere: [f32; 4],
    view: [f32; 4],
    wet: [f32; 4],
}

/// The level pipeline, its bind group layout, the noise sampler and the uniform buffer.
pub(crate) struct LevelPipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    noise_sampler: wgpu::Sampler,
    grab_sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
}

/// The current camera's level textures and placement.
pub(crate) struct LevelTextures {
    image: wgpu::TextureView,
    palette_texture: wgpu::Texture,
    palette: wgpu::TextureView,
    noise: wgpu::TextureView,
    /// The room's `RainMask_` texture (1x1 black without one).
    rain_mask: wgpu::TextureView,
    /// The room's pixel size (`PixelWidth`, `PixelHeight`) from the mask's tiles.
    room_size: Option<[f32; 2]>,
    /// Room position and size of the image.
    rect: [f32; 4],
    params: LevelParams,
}

impl LevelTextures {
    /// Room position and size of the image.
    pub(crate) fn rect(&self) -> [f32; 4] {
        self.rect
    }

    /// The room's shader globals.
    pub(crate) fn params(&self) -> &LevelParams {
        &self.params
    }

    /// `_fogAmount` of the palette.
    pub(crate) fn fog_amount(&self) -> f32 {
        self.params.fog_amount
    }

    /// Rewrites the palette (`ApplyFade` after the fade coordinate changed) and, as its
    /// closing `ApplyPalette` does, `_fogAmount` from the new texture.
    pub(crate) fn set_palette(&mut self, queue: &wgpu::Queue, palette: &rw_assets::PaletteTexture) {
        self.params.fog_amount = palette.fog_amount();
        let width = rw_assets::PaletteTexture::WIDTH as u32;
        let height = rw_assets::PaletteTexture::HEIGHT as u32;
        queue.write_texture(
            self.palette_texture.as_image_copy(),
            &palette.bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// The image, palette and noise views.
    pub(crate) fn views(&self) -> [&wgpu::TextureView; 3] {
        [&self.image, &self.palette, &self.noise]
    }

    pub(crate) fn rain_mask(&self) -> &wgpu::TextureView {
        &self.rain_mask
    }

    /// `_camInRoomRect` (`RoomCamera.DrawUpdate`): the window `view` in room uv,
    /// `(targetPoint / PixelSize, sSize / PixelSize)`; zero without a room mask.
    pub(crate) fn cam_in_room_rect(&self, view: &LevelView) -> [f32; 4] {
        self.room_size.map_or([0.0; 4], |[w, h]| {
            [
                view.origin[0] / w,
                view.origin[1] / h,
                view.size[0] / w,
                view.size[1] / h,
            ]
        })
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
                Self::texture_entry(5, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
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
        // `_NoiseTex` import settings: bilinear (the nearest mip level), repeat; the level shader
        // reads mip 0.
        let noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("noise"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let grab_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("level grab"),
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
            grab_sampler,
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
    fn texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        size: (usize, usize),
        format: wgpu::TextureFormat,
        data: &[u8],
    ) -> wgpu::Texture {
        Self::texture_mips(device, queue, label, size, format, 1, data)
    }

    /// A 2D texture with `mips` mip levels, `data` holding them from the largest.
    fn texture_mips(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        size: (usize, usize),
        format: wgpu::TextureFormat,
        mips: u32,
        data: &[u8],
    ) -> wgpu::Texture {
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.0 as u32,
                height: size.1 as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        device.create_texture_with_data(
            queue,
            &desc,
            wgpu::util::TextureDataOrder::LayerMajor,
            data,
        )
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
        let palette_texture = Self::texture(
            device,
            queue,
            "palette",
            (
                rw_assets::PaletteTexture::WIDTH,
                rw_assets::PaletteTexture::HEIGHT,
            ),
            wgpu::TextureFormat::Rgba8Unorm,
            &level.palette.bytes(),
        );
        let view = wgpu::TextureViewDescriptor::default();
        let (mask_size, mask_bytes): ((usize, usize), &[u8]) = match level.rain_mask {
            Some(m) => ((m.width() as usize, m.height() as usize), m.bytes()),
            None => ((1, 1), &[0, 0, 0, 255]),
        };
        LevelTextures {
            rain_mask: Self::texture(
                device,
                queue,
                "rain mask",
                mask_size,
                wgpu::TextureFormat::Rgba8Unorm,
                mask_bytes,
            )
            .create_view(&view),
            room_size: level.rain_mask.map(|m| {
                [
                    m.width() as f32 * rw_assets::TILE_SIZE,
                    m.height() as f32 * rw_assets::TILE_SIZE,
                ]
            }),
            palette: palette_texture.create_view(&view),
            palette_texture,
            image: Self::texture(
                device,
                queue,
                "level image",
                (image.width(), image.height()),
                wgpu::TextureFormat::Rgba8Unorm,
                &image.rgba_bottom_up(),
            )
            .create_view(&view),
            noise: Self::texture_mips(
                device,
                queue,
                "noise",
                (noise_size, noise_size),
                wgpu::TextureFormat::R8Unorm,
                level.noise.mip_bytes().count() as u32,
                &level
                    .noise
                    .mip_bytes()
                    .flatten()
                    .copied()
                    .collect::<Vec<u8>>(),
            )
            .create_view(&view),
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
            shading: [view.rain, view.light, p.grime, p.fog_amount],
            grade: [p.darkness, p.contrast, p.saturation, p.hue],
            extra: [p.brightness, p.clouds_speed, p.swarm_room, p.rim_fix],
            light_dir: [p.light_dir[0], p.light_dir[1], 1.0 / 1400.0, 1.0 / 800.0],
            atmosphere: [p.atmosphere[0], p.atmosphere[1], p.atmosphere[2], 1.0],
            view: [view.origin[0], view.origin[1], view.size[0], view.size[1]],
            wet: [p.wet_terrain, view.water_level, 0.0, 0.0],
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
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&self.grab_sampler),
                },
            ],
        })
    }
}
