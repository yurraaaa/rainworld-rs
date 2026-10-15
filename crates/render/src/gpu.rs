//! [`Renderer`] on wgpu, pinned to the Vulkan backend.
//!
//! Everything is drawn into an offscreen frame of the game's screen size, without
//! antialiasing (the game's quality level has `antiAliasing: 0` and Futile's cameras
//! `allowMSAA = false`). With a window, [`Renderer::present`] then shows that frame in
//! the window's [`PresentRect`] (`present_gpu`). Screenshots read the frame itself.
//! One pipeline draws coloured triangle lists: the room mesh and grid are uploaded
//! once per [`Renderer::set_room`], the per-frame shapes every [`Renderer::present`].
//! A second one draws the level graphic (`level_gpu`): the shapes and sprites queued
//! before [`Renderer::draw_level`] go to an offscreen "behind" target first, which the
//! level shader reads to let them show through sky and far pixels. A third draws the
//! atlas sprites (`sprite_gpu`), a fourth the game's water (`water_gpu`), which
//! reads the level textures and the "behind" target too, and a fifth the stage quads
//! (`stage_gpu`: the menu shaders, one texture per draw). Draw calls keep their call
//! order across pipelines ([`DrawQueue`]). The WGSL shaders are compiled at runtime by
//! wgpu.
//!
//! [`WgpuRenderer::headless`] has no window: it only draws the frame.

use std::borrow::Cow;
use std::collections::HashSet;
use std::path::PathBuf;

use rw_assets::{PngImage, Room};
use rw_graphics::{DrawList, Layer};
use rw_sim::Vec2;
use wgpu::util::DeviceExt;

use crate::level_gpu::{LevelPipeline, LevelTextures};
use crate::present_gpu::PresentPipeline;
use crate::sprite_gpu::SpritePipeline;
use crate::stage_gpu::StagePipeline;
use crate::water_gpu::{WaterBinding, WaterPipeline};
use crate::{
    Batch, Camera, Color, DrawQueue, FrameImage, LevelGraphics, LevelView, Mesh, PresentRect,
    RenderError, Renderer, Resolution, RoomDebugGeometry, RoomLayers, SpriteAtlas, StageBatch,
    StageGlobals, StageTextureId, TextureSampling, Vertex, WaterSprites,
};

const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.06,
    g: 0.06,
    b: 0.09,
    a: 1.0,
};

/// A static vertex buffer and its vertex count.
struct StaticMesh {
    buffer: wgpu::Buffer,
    count: u32,
}

/// wgpu (Vulkan) implementation of [`Renderer`].
pub struct WgpuRenderer {
    /// `None` when headless.
    surface: Option<wgpu::Surface<'static>>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// The window surface's configuration (format and window size); when headless only
    /// its format is used.
    config: wgpu::SurfaceConfiguration,
    /// The frame everything is drawn to, at [`Renderer::size`].
    frame: FrameTargets,
    present_pipeline: PresentPipeline,
    adapter_info: wgpu::AdapterInfo,
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    level_pipeline: LevelPipeline,
    level: Option<LevelTextures>,
    /// This frame's level placement.
    level_view: Option<LevelView>,
    sprite_pipeline: SpritePipeline,
    water_pipeline: WaterPipeline,
    stage_pipeline: StagePipeline,
    /// The screen size stage quads are placed in this frame.
    stage_screen: [f32; 2],
    /// This frame's clear colour without a level.
    background: wgpu::Color,
    /// This frame's `_waterDepth`.
    water_depth: f32,
    atlas: Option<SpriteAtlas>,
    /// This frame's camera position for sprites (stage origin in room pixels).
    stage_origin: [f32; 2],
    /// Atlas elements already reported missing.
    missing: HashSet<&'static str>,
    room: Option<StaticMesh>,
    grid: Option<StaticMesh>,
    queue_draws: DrawQueue,
    dynamic_buffer: Option<wgpu::Buffer>,
    camera: Camera,
    layers: Option<RoomLayers>,
    /// Where to save the next presented frame, if requested.
    capture: Option<Capture>,
    /// The frame read back by a [`Capture::Memory`].
    captured: Option<FrameImage>,
}

enum Capture {
    File(PathBuf),
    Memory,
}

/// The offscreen frame and the target the shapes behind the level are drawn to, both of
/// the frame's size.
struct FrameTargets {
    size: (u32, u32),
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    behind: wgpu::TextureView,
}

/// A frame copied to a mappable buffer for saving.
struct Readback {
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    /// Bytes per row in `buffer`, padded to `COPY_BYTES_PER_ROW_ALIGNMENT`.
    padded_row: u32,
    /// Source is BGRA rather than RGBA.
    bgra: bool,
}

impl WgpuRenderer {
    /// Creates a renderer drawing frames of `resolution` and showing them scaled in
    /// `target` (e.g. an `Arc<winit::window::Window>`) of physical size `size`, on a
    /// Vulkan adapter. Blocks while the device is created.
    ///
    /// # Errors
    /// [`RenderError::Init`] when the surface, a Vulkan adapter or the device cannot be
    /// created; [`RenderError::NoSurfaceFormat`] when the surface has no format.
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        size: (u32, u32),
        resolution: Resolution,
    ) -> Result<Self, RenderError> {
        let instance = Self::instance();
        let surface = instance
            .create_surface(target)
            .map_err(|e| RenderError::init("create surface", e))?;
        let adapter = Self::adapter(&instance, Some(&surface))?;
        let caps = surface.get_capabilities(&adapter);
        // Colours are authored as display values: prefer a non-sRGB format so they are
        // written unchanged.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or(RenderError::NoSurfaceFormat)?;
        let mut config = surface
            .get_default_config(&adapter, size.0.max(1), size.1.max(1))
            .ok_or(RenderError::NoSurfaceFormat)?;
        config.format = format;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        config.alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        config.view_formats = Vec::new();
        // Lets frames be copied out for screenshots, where supported.
        if caps.usages.contains(wgpu::TextureUsages::COPY_SRC) {
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        log::info!(
            "surface: {:?} {}x{} {:?}",
            config.format,
            config.width,
            config.height,
            config.present_mode
        );
        Self::with_adapter(&adapter, Some(surface), config, resolution.size())
    }

    /// Creates a renderer without a window, drawing `size` frames to an offscreen RGBA
    /// texture on a Vulkan adapter; read them with [`WgpuRenderer::render_image`] and
    /// change the size with [`WgpuRenderer::set_frame_size`].
    ///
    /// # Errors
    /// [`RenderError::Init`] when no Vulkan adapter or device can be created.
    pub fn headless(size: (u32, u32)) -> Result<Self, RenderError> {
        let adapter = Self::adapter(&Self::instance(), None)?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format: wgpu::TextureFormat::Rgba8Unorm,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 0,
            height: 0,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: Vec::new(),
        };
        Self::with_adapter(&adapter, None, config, size)
    }

    fn instance() -> wgpu::Instance {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::VULKAN;
        desc.flags = wgpu::InstanceFlags::from_build_config().with_env();
        wgpu::Instance::new(desc)
    }

    fn adapter(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<wgpu::Adapter, RenderError> {
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface,
            ..Default::default()
        }))
        .map_err(|e| RenderError::init("request Vulkan adapter", e))
    }

    /// The device, pipelines and targets for frames of `config`'s format and
    /// `frame_size`, presented to `surface` or only drawn without one.
    fn with_adapter(
        adapter: &wgpu::Adapter,
        surface: Option<wgpu::Surface<'static>>,
        config: wgpu::SurfaceConfiguration,
        frame_size: (u32, u32),
    ) -> Result<Self, RenderError> {
        let adapter_info = adapter.get_info();
        log::info!(
            "adapter: {} | backend: {} | type: {:?} | driver: {} {}",
            adapter_info.name,
            adapter_info.backend,
            adapter_info.device_type,
            adapter_info.driver,
            adapter_info.driver_info,
        );
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rw-render device"),
            ..Default::default()
        }))
        .map_err(|e| RenderError::init("request device", e))?;
        if let Some(surface) = &surface
            && config.width > 0
            && config.height > 0
        {
            surface.configure(&device, &config);
        }
        let format = config.format;

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: std::mem::size_of::<[f32; 4]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera"),
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
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let pipeline = Self::create_pipeline(&device, &bind_group_layout, format);
        let level_pipeline = LevelPipeline::new(&device, format);
        let sprite_pipeline = SpritePipeline::new(&device, format);
        let water_pipeline = WaterPipeline::new(&device, format);
        let stage_pipeline = StagePipeline::new(&device, &queue, format);
        let frame = FrameTargets::new(&device, format, frame_size);
        let mut present_pipeline = PresentPipeline::new(&device, format);
        present_pipeline.bind(&device, &frame.view);

        Ok(Self {
            surface,
            frame,
            present_pipeline,
            device,
            queue,
            adapter_info,
            pipeline,
            camera_buffer,
            camera_bind_group,
            level_pipeline,
            level: None,
            level_view: None,
            sprite_pipeline,
            water_pipeline,
            stage_pipeline,
            stage_screen: [frame_size.0 as f32, frame_size.1 as f32],
            background: BACKGROUND,
            water_depth: 0.0,
            atlas: None,
            stage_origin: [0.0, 0.0],
            missing: HashSet::new(),
            room: None,
            grid: None,
            queue_draws: DrawQueue::default(),
            dynamic_buffer: None,
            camera: Camera::fit([1.0, 1.0], frame_size, 0.0),
            config,
            layers: None,
            capture: None,
            captured: None,
        })
    }

    /// The debug pipeline: coloured triangle lists, alpha blended, no culling.
    fn create_pipeline(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("debug.wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/debug.wgsl"))),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("debug"),
            bind_group_layouts: &[Some(bind_group_layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("debug"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
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
            cache: None,
        })
    }

    /// Records a copy of the frame into a readback buffer, or `None` when its format is
    /// not 8-bit RGBA/BGRA.
    fn copy_frame(&self, encoder: &mut wgpu::CommandEncoder) -> Option<Readback> {
        let bgra = match self.config.format {
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => false,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => true,
            _ => return None,
        };
        let texture = &self.frame.texture;
        let (width, height) = self.frame.size;
        let padded_row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame readback"),
            size: u64::from(padded_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        Some(Readback {
            buffer,
            width,
            height,
            padded_row,
            bgra,
        })
    }

    /// The adapter in use (name, backend, driver).
    #[must_use]
    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        &self.adapter_info
    }

    /// Whether the window has an area to present to.
    fn window_drawable(&self) -> bool {
        self.config.width > 0 && self.config.height > 0
    }

    /// Changes the frame size (the window's frame keeps the game's resolution; MCP
    /// screenshots pick theirs).
    pub fn set_frame_size(&mut self, size: (u32, u32)) {
        if size == self.frame.size {
            return;
        }
        self.frame = FrameTargets::new(&self.device, self.config.format, size);
        self.present_pipeline.bind(&self.device, &self.frame.view);
    }

    /// Draws `batches` (those queued before the level) into the "behind" target,
    /// cleared to black (the game's screen before the level is drawn).
    fn draw_behind(&self, encoder: &mut wgpu::CommandEncoder, batches: &[Batch]) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("behind level pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.frame.behind,
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
        self.draw_batches(&mut pass, batches, None);
    }

    /// Draws shape, sprite and water batches in order (the level marker is skipped;
    /// water only with its bind group).
    fn draw_batches(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        batches: &[Batch],
        water: Option<&wgpu::BindGroup>,
    ) {
        for batch in batches {
            match batch {
                Batch::Shapes(range) => {
                    let Some(buffer) = &self.dynamic_buffer else {
                        continue;
                    };
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(range.clone(), 0..1);
                }
                Batch::Sprites(range) => self.sprite_pipeline.draw(pass, range.clone()),
                Batch::Level => {}
                Batch::Stage(draw) => self.stage_pipeline.draw(pass, draw),
                Batch::Water(shader, range) => {
                    if let Some(bind_group) = water {
                        self.water_pipeline
                            .draw(pass, bind_group, *shader, range.clone());
                    }
                }
            }
        }
    }

    /// Frame rectangle `(x, y, width, height)` of the room rectangle `view`, clamped to
    /// the frame.
    fn scissor(&self, view: &LevelView) -> Option<(u32, u32, u32, u32)> {
        let top_right = [view.origin[0] + view.size[0], view.origin[1] + view.size[1]];
        let a = self.camera.world_to_screen(view.origin);
        let b = self.camera.world_to_screen(top_right);
        let (w, h) = (self.frame.size.0 as f32, self.frame.size.1 as f32);
        let x0 = a[0].min(b[0]).round().clamp(0.0, w);
        let x1 = a[0].max(b[0]).round().clamp(0.0, w);
        let y0 = a[1].min(b[1]).round().clamp(0.0, h);
        let y1 = a[1].max(b[1]).round().clamp(0.0, h);
        (x1 > x0 && y1 > y0).then_some((x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32))
    }

    fn upload(&self, label: &str, mesh: &Mesh) -> Option<StaticMesh> {
        if mesh.is_empty() {
            return None;
        }
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(mesh.vertices()),
                usage: wgpu::BufferUsages::VERTEX,
            });
        Some(StaticMesh {
            buffer,
            count: mesh.vertex_count(),
        })
    }

    /// Writes the per-frame shapes into the dynamic buffer, growing it as needed.
    fn upload_dynamic(&mut self) {
        let bytes: &[u8] = bytemuck::cast_slice(self.queue_draws.shapes.vertices());
        if bytes.is_empty() {
            return;
        }
        let needed = bytes.len() as u64;
        if self
            .dynamic_buffer
            .as_ref()
            .is_none_or(|b| b.size() < needed)
        {
            self.dynamic_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("dynamic shapes"),
                size: needed.next_power_of_two().max(4096),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buffer) = &self.dynamic_buffer {
            self.queue.write_buffer(buffer, 0, bytes);
        }
    }

    /// Acquires the next surface texture, reconfiguring on outdated/lost surfaces.
    /// `Ok(None)` means skip presenting this frame.
    fn acquire(
        &self,
        surface: &wgpu::Surface<'_>,
    ) -> Result<Option<wgpu::SurfaceTexture>, RenderError> {
        match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => Ok(Some(t)),
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                // Use this frame; reconfigure for the next one.
                surface.configure(&self.device, &self.config);
                Ok(Some(t))
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                log::debug!("surface outdated or lost; reconfiguring");
                surface.configure(&self.device, &self.config);
                Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Validation => Err(RenderError::SurfaceValidation),
        }
    }

    /// Shows the frame in the window's letterbox rectangle, if there is a window to
    /// show it in.
    fn present_to_window(&self) -> Result<(), RenderError> {
        let Some(surface) = &self.surface else {
            return Ok(());
        };
        let window = (self.config.width, self.config.height);
        let Some(rect) = PresentRect::fit(self.frame.size, window) else {
            return Ok(());
        };
        let Some(texture) = self.acquire(surface)? else {
            return Ok(());
        };
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("present"),
            });
        self.present_pipeline.draw(&mut encoder, &view, rect);
        self.queue.submit([encoder.finish()]);
        self.queue.present(texture);
        Ok(())
    }

    /// Renders the queued frame (presenting it too when there is a window) and returns
    /// its pixels at the frame size.
    ///
    /// # Errors
    /// As [`Renderer::present`]; [`RenderError::CaptureUnsupported`] when the frame
    /// cannot be read back or the size is zero.
    pub fn render_image(&mut self) -> Result<FrameImage, RenderError> {
        self.capture = Some(Capture::Memory);
        let presented = self.present();
        self.capture = None;
        presented?;
        self.captured.take().ok_or(RenderError::CaptureUnsupported)
    }
}

impl FrameTargets {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat, size: (u32, u32)) -> Self {
        let texture = Self::texture(
            device,
            "frame",
            format,
            size,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        let behind = Self::texture(
            device,
            "behind level",
            format,
            size,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        Self {
            size,
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            texture,
            behind: behind.create_view(&wgpu::TextureViewDescriptor::default()),
        }
    }

    /// A single-sample texture; zero sizes become 1x1 (nothing is drawn then).
    fn texture(
        device: &wgpu::Device,
        label: &str,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }

    fn is_empty(&self) -> bool {
        self.size.0 == 0 || self.size.1 == 0
    }
}

impl Renderer for WgpuRenderer {
    fn size(&self) -> (u32, u32) {
        self.frame.size
    }

    fn resize(&mut self, width: u32, height: u32) {
        if (width, height) == (self.config.width, self.config.height) {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        if let Some(surface) = &self.surface
            && self.window_drawable()
        {
            surface.configure(&self.device, &self.config);
        }
    }

    fn set_room(&mut self, room: &Room) {
        self.room = self.upload("room tiles", &room.debug_mesh());
        self.grid = self.upload("room grid", &room.debug_grid_mesh());
    }

    fn begin_frame(&mut self, camera: Camera) {
        self.camera = camera;
        self.queue_draws.clear();
        self.layers = None;
        self.level_view = None;
        self.stage_origin = [0.0, 0.0];
        self.background = BACKGROUND;
    }

    fn draw_room(&mut self, layers: RoomLayers) {
        self.layers = Some(layers);
    }

    fn set_level(&mut self, level: &LevelGraphics<'_>) {
        self.level = Some(LevelPipeline::upload(&self.device, &self.queue, level));
    }

    fn draw_level(&mut self, view: LevelView) {
        self.level_view = Some(view);
        self.queue_draws.mark_level();
    }

    fn draw_circle(&mut self, centre: [f32; 2], radius: f32, color: Color) {
        self.queue_draws.shapes.circle(centre, radius, color);
    }

    fn draw_ring(&mut self, centre: [f32; 2], radius: f32, width: f32, color: Color) {
        self.queue_draws.shapes.ring(centre, radius, width, color);
    }

    fn draw_line(&mut self, a: [f32; 2], b: [f32; 2], width: f32, color: Color) {
        self.queue_draws.shapes.line(a, b, width, color);
    }

    fn draw_rect(&mut self, min: [f32; 2], max: [f32; 2], color: Color) {
        self.queue_draws.shapes.rect(min, max, color);
    }

    fn draw_mesh(&mut self, mesh: &Mesh) {
        self.queue_draws.shapes.extend(mesh);
    }

    fn draw_water_sprites(&mut self, sprites: &WaterSprites, depth: f32) {
        self.water_depth = depth;
        self.queue_draws.push_water(sprites);
    }

    fn set_stage_texture(
        &mut self,
        id: StageTextureId,
        image: &PngImage,
        sampling: TextureSampling,
    ) {
        self.stage_pipeline
            .set_texture(&self.device, &self.queue, id, image, sampling);
    }

    fn remove_stage_texture(&mut self, id: StageTextureId) {
        self.stage_pipeline.remove_texture(&self.device, id);
    }

    fn set_stage_globals(&mut self, globals: StageGlobals) {
        self.stage_pipeline.set_globals(&self.device, globals);
    }

    fn draw_stage(&mut self, batch: &StageBatch, screen_size: [f32; 2]) {
        self.stage_screen = screen_size;
        self.queue_draws.push_stage(batch);
    }

    fn set_background(&mut self, color: Color) {
        let [r, g, b, a] = color.0.map(f64::from);
        self.background = wgpu::Color { r, g, b, a };
    }

    fn set_atlas(&mut self, atlas: &SpriteAtlas, image: &PngImage) {
        self.sprite_pipeline
            .set_atlas(&self.device, &self.queue, image);
        self.atlas = Some(atlas.clone());
        self.missing.clear();
    }

    fn draw_sprites(&mut self, list: &DrawList, layer: Layer, cam_pos: [f32; 2]) {
        let Some(atlas) = &self.atlas else { return };
        self.stage_origin = cam_pos;
        let missing = self
            .queue_draws
            .push_sprites(atlas, list, layer, Vec2::from(cam_pos));
        for name in missing {
            if self.missing.insert(name) {
                log::warn!("atlas has no element {name}; sprite not drawn");
            }
        }
    }

    fn present(&mut self) -> Result<(), RenderError> {
        if self.frame.is_empty() {
            return Ok(());
        }
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&self.camera.clip_transform()),
        );
        self.upload_dynamic();
        self.sprite_pipeline.upload(
            &self.device,
            &self.queue,
            &self.queue_draws.sprites,
            &self.camera,
            self.stage_origin,
        );
        self.water_pipeline
            .upload(&self.device, &self.queue, &self.queue_draws.water);
        let stage_camera = Camera::window([0.0, 0.0], self.stage_screen, self.frame.size);
        self.stage_pipeline.upload(
            &self.device,
            &self.queue,
            &self.queue_draws.stage.vertices,
            &self.queue_draws.stage.indices,
            &stage_camera,
            self.stage_screen,
        );
        let batches = self.queue_draws.batches();
        let split = batches.iter().position(|b| *b == Batch::Level);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        // The level, when drawn: its bind group, scissor and the batches behind it.
        let level = match (&self.level_view, split, &self.level) {
            (Some(view), Some(split), Some(textures)) => {
                self.draw_behind(&mut encoder, &batches[..split]);
                let bind_group = self.level_pipeline.bind(
                    &self.device,
                    &self.queue,
                    textures,
                    &self.frame.behind,
                    &self.camera,
                    view,
                );
                let water = self.water_pipeline.bind(
                    &self.device,
                    &self.queue,
                    textures,
                    &WaterBinding {
                        behind: &self.frame.behind,
                        camera: &self.camera,
                        view,
                        depth: self.water_depth,
                        frame_size: self.frame.size,
                    },
                );
                Some((bind_group, self.scissor(view), split + 1, water))
            }
            _ => None,
        };
        let clear = if level.is_some() {
            wgpu::Color::BLACK
        } else {
            self.background
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.frame.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut first_batch = 0;
            if let Some((bind_group, scissor, after_level, _)) = &level {
                let (x, y, w, h) = scissor.unwrap_or_default();
                pass.set_scissor_rect(x, y, w, h);
                pass.set_pipeline(self.level_pipeline.pipeline());
                pass.set_bind_group(0, bind_group, &[]);
                pass.draw(0..6, 0..1);
                first_batch = *after_level;
            }
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            let layers = self.layers.unwrap_or(RoomLayers {
                tiles: false,
                grid: false,
            });
            let statics = [
                (layers.tiles, self.room.as_ref()),
                (layers.grid, self.grid.as_ref()),
            ];
            for mesh in statics
                .into_iter()
                .filter(|(on, _)| *on)
                .filter_map(|(_, m)| m)
            {
                pass.set_vertex_buffer(0, mesh.buffer.slice(..));
                pass.draw(0..mesh.count, 0..1);
            }
            let water = level.as_ref().map(|(_, _, _, water)| water);
            self.draw_batches(&mut pass, &batches[first_batch..], water);
        }
        let readback = self
            .capture
            .is_some()
            .then(|| self.copy_frame(&mut encoder))
            .flatten();
        self.queue.submit([encoder.finish()]);
        self.present_to_window()?;
        if let Some(capture) = self.capture.take() {
            let readback = readback.ok_or(RenderError::CaptureUnsupported)?;
            match capture {
                Capture::File(path) => readback
                    .read(&self.device)
                    .and_then(|image| image.save_ppm(&path))
                    .map_err(|source| RenderError::Capture { path, source })?,
                Capture::Memory => {
                    let image = readback.read(&self.device).map_err(RenderError::Readback)?;
                    self.captured = Some(image);
                }
            }
        }
        Ok(())
    }

    fn capture_next_frame(&mut self, path: PathBuf) {
        self.capture = Some(Capture::File(path));
    }
}

impl Readback {
    /// Waits for the copy and returns the frame as opaque RGBA.
    fn read(&self, device: &wgpu::Device) -> std::io::Result<FrameImage> {
        let slice = self.buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            if let Err(e) = result {
                log::error!("frame readback map failed: {e}");
            }
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(std::io::Error::other)?;
        let data = slice.get_mapped_range().map_err(std::io::Error::other)?;
        let mut rgba = Vec::with_capacity(self.width as usize * self.height as usize * 4);
        for row in data.chunks_exact(self.padded_row as usize) {
            for px in row[..self.width as usize * 4].as_chunks::<4>().0 {
                let rgba_px = if self.bgra {
                    [px[2], px[1], px[0], 255]
                } else {
                    [px[0], px[1], px[2], 255]
                };
                rgba.extend_from_slice(&rgba_px);
            }
        }
        Ok(FrameImage::new(self.width, self.height, rgba))
    }
}
