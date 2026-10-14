//! GPU side of the stage: a `stage.wgsl` pipeline per game shader, one bind group per
//! uploaded texture, the shader globals and the frame's stage buffers.

use std::borrow::Cow;
use std::collections::HashMap;

use rw_assets::PngImage;
use wgpu::util::DeviceExt;

use crate::{
    Camera, SpriteVertex, StageDraw, StageGlobals, StageShader, StageTextureId, TextureFilter,
    TextureSampling,
};

/// An uploaded stage texture.
struct StageTexture {
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
}

pub(crate) struct StagePipeline {
    pipelines: HashMap<StageShader, wgpu::RenderPipeline>,
    texture_layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    textures: HashMap<StageTextureId, StageTexture>,
    /// White 1x1, bound as the text gradient until one is set.
    blank: StageTexture,
    globals: StageGlobals,
    /// Group 2: the text gradient.
    globals_bind_group: wgpu::BindGroup,
    vertices: Option<wgpu::Buffer>,
    indices: Option<wgpu::Buffer>,
}

impl StagePipeline {
    /// Each shader and its fragment entry point.
    const SHADERS: [(StageShader, &'static str); 7] = [
        (StageShader::Basic, "fs_basic"),
        (StageShader::EdgeFade, "fs_edge_fade"),
        (StageShader::FlatLight, "fs_flat_light"),
        (StageShader::VectorCircle, "fs_vector_circle"),
        (StageShader::VectorCircleFadable, "fs_vector_circle_fadable"),
        (StageShader::HoldButtonCircle, "fs_hold_button_circle"),
        (StageShader::MenuText, "fs_menu_text"),
    ];

    /// The stage uniform: transform, screen size, `_RAIN` and padding.
    const UNIFORM_FLOATS: usize = 8;

    /// `Blend SrcAlpha OneMinusSrcAlpha`, as every stage shader.
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
    ) -> Self {
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stage uniform"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
            bind_group_layouts: &[
                Some(&uniform_layout),
                Some(&texture_layout),
                Some(&texture_layout),
            ],
            immediate_size: 0,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("stage uniform"),
            size: std::mem::size_of::<[f32; Self::UNIFORM_FLOATS]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stage uniform"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let pipelines = Self::SHADERS
            .iter()
            .map(|&(id, entry)| (id, Self::pipeline(device, &layout, &shader, format, entry)))
            .collect();
        let blank = Self::create_texture(
            device,
            queue,
            &texture_layout,
            &PngImage::from_pixels(1, 1, vec![[255; 4]]),
            TextureSampling {
                filter: TextureFilter::Point,
                clamp: true,
            },
        );
        let globals_bind_group = Self::globals_group(device, &texture_layout, &blank);
        Self {
            pipelines,
            texture_layout,
            uniform,
            uniform_bind_group,
            textures: HashMap::new(),
            blank,
            globals: StageGlobals::default(),
            globals_bind_group,
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
        let texture = Self::create_texture(device, queue, &self.texture_layout, image, sampling);
        self.textures.insert(id, texture);
        if self.globals.text_gradient == Some(id) {
            self.rebind_globals(device);
        }
    }

    fn create_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        image: &PngImage,
        sampling: TextureSampling,
    ) -> StageTexture {
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
            layout,
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
        StageTexture {
            view,
            sampler,
            bind_group,
        }
    }

    fn globals_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        gradient: &StageTexture,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stage globals"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&gradient.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&gradient.sampler),
                },
            ],
        })
    }

    /// Sets the globals; the text gradient is bound once its texture is uploaded.
    pub(crate) fn set_globals(&mut self, device: &wgpu::Device, globals: StageGlobals) {
        let rebind = globals.text_gradient != self.globals.text_gradient;
        self.globals = globals;
        if rebind {
            self.rebind_globals(device);
        }
    }

    fn rebind_globals(&mut self, device: &wgpu::Device) {
        let gradient = self
            .globals
            .text_gradient
            .and_then(|id| self.textures.get(&id))
            .unwrap_or(&self.blank);
        self.globals_bind_group = Self::globals_group(device, &self.texture_layout, gradient);
    }

    pub(crate) fn remove_texture(&mut self, device: &wgpu::Device, id: StageTextureId) {
        self.textures.remove(&id);
        if self.globals.text_gradient == Some(id) {
            self.rebind_globals(device);
        }
    }

    /// Writes the frame's stage quads, the screen camera and the globals.
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[SpriteVertex],
        indices: &[u32],
        camera: &Camera,
        screen: [f32; 2],
    ) {
        let [a, b, c, d] = camera.clip_transform();
        let uniform: [f32; Self::UNIFORM_FLOATS] =
            [a, b, c, d, screen[0], screen[1], self.globals.rain, 0.0];
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&uniform));
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
        let (Some(texture), Some(pipeline), Some(vertices), Some(indices)) = (
            self.textures.get(&draw.texture),
            self.pipelines.get(&draw.shader),
            &self.vertices,
            &self.indices,
        ) else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        pass.set_bind_group(1, &texture.bind_group, &[]);
        pass.set_bind_group(2, &self.globals_bind_group, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(draw.indices.clone(), 0, 0..1);
    }
}
