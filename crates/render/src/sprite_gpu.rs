//! GPU side of the room sprites: a `sprite.wgsl` pipeline per game shader, a texture
//! per atlas page, the room globals, `_NoiseTex2`, the screen-sized textures some
//! shaders read (level image, what was drawn behind the level, the frame grab, the
//! palette) and the per-frame sprite buffers.
//!
//! Mirrors: -

use std::borrow::Cow;

use rustc_hash::FxHashMap;
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
    frame: [f32; 2],
    fog_amount: f32,
    water_level: f32,
}

/// The sprite pipelines, the atlas pages' bind groups and the frame's buffers.
pub(crate) struct SpritePipeline {
    pipelines: FxHashMap<Shader, wgpu::RenderPipeline>,
    layout: wgpu::BindGroupLayout,
    noise_layout: wgpu::BindGroupLayout,
    screen_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// `_NoiseTex`'s import settings.
    noise_sampler: wgpu::Sampler,
    /// The full-screen effects' frame grab: bilinear, clamped.
    grab_sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    /// One per atlas page, set by [`SpritePipeline::set_atlas`].
    pages: Vec<wgpu::BindGroup>,
    /// `_NoiseTex2`, bound each frame by [`SpritePipeline::set_frame`].
    noise_group: Option<wgpu::BindGroup>,
    /// 1x1 black screen textures, for frames without a level and for drawing into the
    /// "behind" target (which cannot be read while it is drawn to).
    blank_screen: wgpu::BindGroup,
    blank: wgpu::TextureView,
    vertices: Option<wgpu::Buffer>,
    indices: Option<wgpu::Buffer>,
}

/// The screen-sized textures of one frame.
pub(crate) struct ScreenTextures<'a> {
    pub level: &'a wgpu::TextureView,
    pub behind: &'a wgpu::TextureView,
    pub grab: &'a wgpu::TextureView,
    pub palette: &'a wgpu::TextureView,
    /// `_NoiseTex` (`Palettes/noise`).
    pub noise: &'a wgpu::TextureView,
}

impl SpritePipeline {
    /// Each shader and its fragment entry point.
    const SHADERS: [(Shader, &'static str); 15] = [
        (Shader::Basic, "fs_basic"),
        (Shader::FlatLight, "fs_flat_light"),
        (Shader::OverseerZip, "fs_overseer_zip"),
        (Shader::Hologram, "fs_hologram"),
        (Shader::LightSource, "fs_light_source"),
        (Shader::HoloGrid, "fs_holo_grid"),
        (Shader::HologramImage, "fs_hologram_image"),
        (Shader::ShortCut0, "fs_shortcut"),
        (Shader::ColoredSprite3, "fs_colored_sprite3"),
        (Shader::UnderWaterLight, "fs_under_water_light"),
        (Shader::Bloom, "fs_bloom"),
        (Shader::SkyBloom, "fs_sky_bloom"),
        (Shader::LightBloom, "fs_light_bloom"),
        (Shader::LightAndSkyBloom, "fs_light_and_sky_bloom"),
        (Shader::Fog, "fs_fog"),
    ];

    /// `_hologramThreshold` at render scale 1 (`RainWorld`, decompile 110635: 0.5 when
    /// `Futile.screen.renderScale > 1`).
    const HOLOGRAM_THRESHOLD: f32 = 0.65;

    /// `Blend SrcAlpha OneMinusSrcAlpha` on colour and alpha, as all these shaders.
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
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        cache: &wgpu::PipelineCache,
    ) -> Self {
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
                Self::texture_entry(1, false),
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
                Self::texture_entry(0, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let screen_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite screen textures"),
            entries: &[
                Self::texture_entry(0, false),
                Self::texture_entry(1, false),
                Self::texture_entry(2, true),
                Self::texture_entry(3, false),
                Self::texture_entry(4, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite.wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(concat!(
                include_str!("../shaders/sprite.wgsl"),
                include_str!("../shaders/full_screen.wgsl"),
            ))),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprites"),
            bind_group_layouts: &[Some(&layout), Some(&noise_layout), Some(&screen_layout)],
            immediate_size: 0,
        });
        let pipelines = Self::SHADERS
            .iter()
            .map(|&(id, entry)| {
                let pipeline =
                    Self::pipeline(device, &pipeline_layout, cache, &shader, format, entry);
                (id, pipeline)
            })
            .collect();
        // Atlas import settings: point filter, clamp, no mips (the projections are
        // loaded crisp and clamped too).
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        // `_NoiseTex` import settings: bilinear, repeat (mip 0 only).
        let noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite noise"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let grab_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite grab"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite room uniform"),
            size: std::mem::size_of::<RoomUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let blank = device
            .create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("blank screen texture"),
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &[0, 0, 0, 255],
            )
            .create_view(&wgpu::TextureViewDescriptor::default());
        let blank_screen = Self::screen_group(
            device,
            &screen_layout,
            [&noise_sampler, &grab_sampler],
            &ScreenTextures {
                level: &blank,
                behind: &blank,
                grab: &blank,
                palette: &blank,
                noise: &blank,
            },
        );
        Self {
            pipelines,
            layout,
            noise_layout,
            screen_layout,
            sampler,
            noise_sampler,
            grab_sampler,
            uniform,
            pages: Vec::new(),
            noise_group: None,
            blank_screen,
            blank,
            vertices: None,
            indices: None,
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

    fn pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        cache: &wgpu::PipelineCache,
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
            cache: Some(cache),
        })
    }

    /// Uploads each atlas page's image as RGBA8 unorm (gamma-space values used as
    /// stored), bottom row first so Futile's texture coordinates address it directly.
    pub(crate) fn set_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        images: &[&PngImage],
    ) {
        self.pages = images
            .iter()
            .map(|image| self.page_group(device, queue, image))
            .collect();
    }

    fn page_group(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &PngImage,
    ) -> wgpu::BindGroup {
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
        device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        })
    }

    /// `samplers`: `_NoiseTex`'s and the grab's.
    fn screen_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        samplers: [&wgpu::Sampler; 2],
        textures: &ScreenTextures<'_>,
    ) -> wgpu::BindGroup {
        let views = [
            textures.level,
            textures.behind,
            textures.grab,
            textures.palette,
            textures.noise,
        ];
        let mut entries: Vec<wgpu::BindGroupEntry<'_>> = views
            .iter()
            .enumerate()
            .map(|(i, view)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu::BindingResource::TextureView(view),
            })
            .collect();
        for (i, sampler) in samplers.into_iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: 5 + i as u32,
                resource: wgpu::BindingResource::Sampler(sampler),
            });
        }
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite screen textures"),
            layout,
            entries: &entries,
        })
    }

    /// The screen textures of a frame with a level.
    pub(crate) fn bind_screen(
        &self,
        device: &wgpu::Device,
        textures: &ScreenTextures<'_>,
    ) -> wgpu::BindGroup {
        Self::screen_group(
            device,
            &self.screen_layout,
            [&self.noise_sampler, &self.grab_sampler],
            textures,
        )
    }

    /// The screen textures for drawing into the "behind" target: the level image and
    /// palette (the game's `_LevelTex` and `_PalTex` are global, so `ShortCut0` sprites
    /// behind the level read it), black for the targets that cannot be read yet.
    pub(crate) fn bind_behind_screen(
        &self,
        device: &wgpu::Device,
        level: &wgpu::TextureView,
        palette: &wgpu::TextureView,
        noise: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        Self::screen_group(
            device,
            &self.screen_layout,
            [&self.noise_sampler, &self.grab_sampler],
            &ScreenTextures {
                level,
                behind: &self.blank,
                grab: &self.blank,
                palette,
                noise,
            },
        )
    }

    /// Black stand-ins for the screen textures.
    pub(crate) fn blank_screen(&self) -> &wgpu::BindGroup {
        &self.blank_screen
    }

    /// Writes the room globals and the stage camera (`camera` shifted so stage
    /// positions, room minus `cam_pos`, land where their room positions would), and
    /// binds `noise2` (view and sampler) as `_NoiseTex2`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn set_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        cam_pos: [f32; 2],
        globals: &RoomGlobals,
        frame_size: (u32, u32),
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
            frame: [frame_size.0 as f32, frame_size.1 as f32],
            fog_amount: globals.fog_amount,
            water_level: globals.water_level,
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

    /// Draws one draw of the uploaded batch with its shader and page, reading `screen`.
    pub(crate) fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        draw: &SpriteDraw,
        screen: &wgpu::BindGroup,
    ) {
        let (Some(pipeline), Some(page), Some(noise), Some(vertices), Some(indices)) = (
            self.pipelines.get(&draw.shader),
            self.pages.get(draw.page),
            &self.noise_group,
            &self.vertices,
            &self.indices,
        ) else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, page, &[]);
        pass.set_bind_group(1, noise, &[]);
        pass.set_bind_group(2, screen, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(draw.indices.clone(), 0, 0..1);
    }
}
