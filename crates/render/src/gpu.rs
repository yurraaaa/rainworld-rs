//! [`Renderer`] on wgpu, pinned to the Vulkan backend.
//!
//! One pipeline draws coloured triangle lists: the room mesh and grid are uploaded
//! once per [`Renderer::set_room`], the per-frame shapes every [`Renderer::present`].
//! A second one draws the level graphic (`level_gpu`): the shapes queued before
//! [`Renderer::draw_level`] go to an offscreen "behind" target first, which the level
//! shader reads to let them show through sky and far pixels. The WGSL shaders are
//! compiled at runtime by wgpu.

use std::borrow::Cow;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use rw_assets::Room;
use wgpu::util::DeviceExt;

use crate::level_gpu::{LevelPipeline, LevelTextures};
use crate::{
    Camera, Color, LevelGraphics, LevelView, Mesh, RenderError, Renderer, RoomDebugGeometry,
    RoomLayers, Vertex,
};

const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.06,
    g: 0.06,
    b: 0.09,
    a: 1.0,
};

/// Preferred MSAA sample count; falls back to 1 when the format does not support it.
const MSAA_SAMPLES: u32 = 4;

/// A static vertex buffer and its vertex count.
struct StaticMesh {
    buffer: wgpu::Buffer,
    count: u32,
}

/// wgpu (Vulkan) implementation of [`Renderer`].
pub struct WgpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    adapter_info: wgpu::AdapterInfo,
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    sample_count: u32,
    msaa_view: Option<wgpu::TextureView>,
    /// Single-sample target the shapes behind the level are drawn to.
    behind_view: Option<wgpu::TextureView>,
    level_pipeline: LevelPipeline,
    level: Option<LevelTextures>,
    /// This frame's level placement and the number of dynamic vertices queued before it.
    level_draw: Option<(LevelView, u32)>,
    room: Option<StaticMesh>,
    grid: Option<StaticMesh>,
    dynamic: Mesh,
    dynamic_buffer: Option<wgpu::Buffer>,
    camera: Camera,
    layers: Option<RoomLayers>,
    /// Where to save the next presented frame, if requested.
    capture: Option<PathBuf>,
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
    /// Creates a renderer drawing to `target` (e.g. an `Arc<winit::window::Window>`)
    /// of physical size `size`, on a Vulkan adapter. Blocks while the device is created.
    ///
    /// # Errors
    /// [`RenderError::Init`] when the surface, a Vulkan adapter or the device cannot be
    /// created; [`RenderError::NoSurfaceFormat`] when the surface has no format.
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        size: (u32, u32),
    ) -> Result<Self, RenderError> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::VULKAN;
        desc.flags = wgpu::InstanceFlags::from_build_config().with_env();
        let instance = wgpu::Instance::new(desc);
        let surface = instance
            .create_surface(target)
            .map_err(|e| RenderError::init("create surface", e))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| RenderError::init("request Vulkan adapter", e))?;
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
        if size.0 > 0 && size.1 > 0 {
            surface.configure(&device, &config);
        }

        let sample_count = if adapter
            .get_texture_format_features(format)
            .flags
            .sample_count_supported(MSAA_SAMPLES)
        {
            MSAA_SAMPLES
        } else {
            1
        };

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
        let pipeline = Self::create_pipeline(&device, &bind_group_layout, format, sample_count);
        let level_pipeline = LevelPipeline::new(&device, format, sample_count);

        let mut renderer = Self {
            surface,
            device,
            queue,
            config,
            adapter_info,
            pipeline,
            camera_buffer,
            camera_bind_group,
            sample_count,
            msaa_view: None,
            behind_view: None,
            level_pipeline,
            level: None,
            level_draw: None,
            room: None,
            grid: None,
            dynamic: Mesh::new(),
            dynamic_buffer: None,
            camera: Camera::fit([1.0, 1.0], size, 0.0),
            layers: None,
            capture: None,
        };
        renderer.recreate_msaa();
        Ok(renderer)
    }

    /// The debug pipeline: coloured triangle lists, alpha blended, no culling.
    fn create_pipeline(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("debug.wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("debug.wgsl"))),
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
            multisample: wgpu::MultisampleState {
                count: sample_count,
                ..Default::default()
            },
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

    /// Records a copy of `texture` (the frame) into a readback buffer, or `None` when
    /// the surface cannot be copied from or its format is not 8-bit RGBA/BGRA.
    fn copy_frame(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
    ) -> Option<Readback> {
        let bgra = match self.config.format {
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => false,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => true,
            _ => return None,
        };
        if !self.config.usage.contains(wgpu::TextureUsages::COPY_SRC) {
            return None;
        }
        let (width, height) = (self.config.width, self.config.height);
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

    fn is_drawable(&self) -> bool {
        self.config.width > 0 && self.config.height > 0
    }

    /// Recreates the MSAA and "behind" targets at the surface size.
    fn recreate_msaa(&mut self) {
        let drawable = self.is_drawable();
        self.msaa_view = (self.sample_count > 1 && drawable).then(|| {
            self.target(
                "msaa",
                self.sample_count,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
        });
        self.behind_view = drawable.then(|| {
            self.target(
                "behind level",
                1,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            )
        });
    }

    /// A render target of the surface's size and format.
    fn target(&self, label: &str, samples: u32, usage: wgpu::TextureUsages) -> wgpu::TextureView {
        self.device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Draws the first `count` dynamic vertices into the "behind" target, cleared to
    /// black (the game's screen before the level is drawn).
    fn draw_behind(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        behind: &wgpu::TextureView,
        count: u32,
    ) {
        let (target, resolve) = match &self.msaa_view {
            Some(msaa) => (msaa, Some(behind)),
            None => (behind, None),
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("behind level pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: resolve,
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
        if let (Some(buffer), true) = (&self.dynamic_buffer, count > 0) {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..count, 0..1);
        }
    }

    /// Screen rectangle `(x, y, width, height)` of the room rectangle `view`, clamped to
    /// the surface.
    fn scissor(&self, view: &LevelView) -> Option<(u32, u32, u32, u32)> {
        let top_right = [view.origin[0] + view.size[0], view.origin[1] + view.size[1]];
        let a = self.camera.world_to_screen(view.origin);
        let b = self.camera.world_to_screen(top_right);
        let (w, h) = (self.config.width as f32, self.config.height as f32);
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
        let bytes: &[u8] = bytemuck::cast_slice(self.dynamic.vertices());
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
    /// `Ok(None)` means skip this frame.
    fn acquire(&mut self) -> Result<Option<wgpu::SurfaceTexture>, RenderError> {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => Ok(Some(t)),
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                // Use this frame; reconfigure for the next one.
                self.surface.configure(&self.device, &self.config);
                Ok(Some(t))
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                log::debug!("surface outdated or lost; reconfiguring");
                self.surface.configure(&self.device, &self.config);
                Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Validation => Err(RenderError::SurfaceValidation),
        }
    }
}

impl Renderer for WgpuRenderer {
    fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    fn resize(&mut self, width: u32, height: u32) {
        if (width, height) == self.size() {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        if self.is_drawable() {
            self.surface.configure(&self.device, &self.config);
        }
        self.recreate_msaa();
    }

    fn set_room(&mut self, room: &Room) {
        self.room = self.upload("room tiles", &room.debug_mesh());
        self.grid = self.upload("room grid", &room.debug_grid_mesh());
    }

    fn begin_frame(&mut self, camera: Camera) {
        self.camera = camera;
        self.dynamic.clear();
        self.layers = None;
        self.level_draw = None;
    }

    fn draw_room(&mut self, layers: RoomLayers) {
        self.layers = Some(layers);
    }

    fn set_level(&mut self, level: &LevelGraphics<'_>) {
        self.level = Some(LevelPipeline::upload(&self.device, &self.queue, level));
    }

    fn draw_level(&mut self, view: LevelView) {
        self.level_draw = Some((view, self.dynamic.vertex_count()));
    }

    fn draw_circle(&mut self, centre: [f32; 2], radius: f32, color: Color) {
        self.dynamic.circle(centre, radius, color);
    }

    fn draw_ring(&mut self, centre: [f32; 2], radius: f32, width: f32, color: Color) {
        self.dynamic.ring(centre, radius, width, color);
    }

    fn draw_line(&mut self, a: [f32; 2], b: [f32; 2], width: f32, color: Color) {
        self.dynamic.line(a, b, width, color);
    }

    fn draw_rect(&mut self, min: [f32; 2], max: [f32; 2], color: Color) {
        self.dynamic.rect(min, max, color);
    }

    fn draw_mesh(&mut self, mesh: &Mesh) {
        self.dynamic.extend(mesh);
    }

    fn present(&mut self) -> Result<(), RenderError> {
        if !self.is_drawable() {
            return Ok(());
        }
        let Some(frame) = self.acquire()? else {
            return Ok(());
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&self.camera.clip_transform()),
        );
        self.upload_dynamic();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        // The level, when drawn: its bind group, scissor and the shapes behind it.
        let level = match (&self.level_draw, &self.level, &self.behind_view) {
            (Some((view, split)), Some(textures), Some(behind)) => {
                self.draw_behind(&mut encoder, behind, *split);
                let bind_group = self.level_pipeline.bind(
                    &self.device,
                    &self.queue,
                    textures,
                    behind,
                    &self.camera,
                    view,
                );
                Some((bind_group, self.scissor(view), *split))
            }
            _ => None,
        };
        let clear = if level.is_some() {
            wgpu::Color::BLACK
        } else {
            BACKGROUND
        };
        {
            let (target, resolve) = match &self.msaa_view {
                Some(msaa) => (msaa, Some(&view)),
                None => (&view, None),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("debug pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: if resolve.is_some() {
                            wgpu::StoreOp::Discard
                        } else {
                            wgpu::StoreOp::Store
                        },
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut first_dynamic = 0;
            if let Some((bind_group, scissor, split)) = &level {
                let (x, y, w, h) = scissor.unwrap_or_default();
                pass.set_scissor_rect(x, y, w, h);
                pass.set_pipeline(self.level_pipeline.pipeline());
                pass.set_bind_group(0, bind_group, &[]);
                pass.draw(0..6, 0..1);
                first_dynamic = *split;
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
            let count = self.dynamic.vertex_count();
            if let (Some(buffer), true) = (&self.dynamic_buffer, count > first_dynamic) {
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(first_dynamic..count, 0..1);
            }
        }
        let readback = self
            .capture
            .is_some()
            .then(|| self.copy_frame(&mut encoder, &frame.texture))
            .flatten();
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        if let Some(path) = self.capture.take() {
            let readback = readback.ok_or(RenderError::CaptureUnsupported)?;
            readback
                .save_ppm(&self.device, &path)
                .map_err(|source| RenderError::Capture { path, source })?;
        }
        Ok(())
    }

    fn capture_next_frame(&mut self, path: PathBuf) {
        self.capture = Some(path);
    }
}

impl Readback {
    /// Waits for the copy and writes the frame as a binary PPM (P6) to `path`.
    fn save_ppm(&self, device: &wgpu::Device, path: &Path) -> std::io::Result<()> {
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
        let mut out = BufWriter::new(File::create(path)?);
        write!(out, "P6\n{} {}\n255\n", self.width, self.height)?;
        for row in data.chunks_exact(self.padded_row as usize) {
            for px in row[..self.width as usize * 4].as_chunks::<4>().0 {
                let rgb = if self.bgra {
                    [px[2], px[1], px[0]]
                } else {
                    [px[0], px[1], px[2]]
                };
                out.write_all(&rgb)?;
            }
        }
        out.flush()
    }
}
