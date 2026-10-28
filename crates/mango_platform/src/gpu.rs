//! Hardware-accelerated GPU Compositor powered by `wgpu`.
//!
//! Merges rasterized UI layers and page tiles directly in GPU memory.
//! Enables 60+ FPS buttery-smooth scrolling with zero CPU re-rasterization during scroll,
//! automatic multi-backend support (Vulkan, Metal, DirectX 12, WebGPU), and resilient
//! swapchain / surface recovery.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};

use bytemuck::{Pod, Zeroable};
use mango_core::{Color, Rect};
use mango_render::layer::LayerTree;
use mango_render::tile::TileCoord;
use winit::window::Window;

// Surface uploads happen serially on the event thread. Reuse one high-water
// staging allocation across textures instead of allocating a full RGBA Vec for
// every tile and every dirty frame.
static PIXEL_UPLOAD_BUFFER: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();

/// Vertex passed to the compositor quad pipeline.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CompositorVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub opacity: f32,
}

impl CompositorVertex {
    pub const fn new(pos: [f32; 2], uv: [f32; 2], opacity: f32) -> Self {
        Self { pos, uv, opacity }
    }
}

/// Backwards-compatible alias for the compositor vertex format.
pub use CompositorVertex as QuadVertex;

/// An allocated GPU 2D texture with its dedicated bind group.
struct GpuTexture {
    texture: wgpu::Texture,
    bind_group: Arc<wgpu::BindGroup>,
    width: u32,
    height: u32,
    version: u64,
}

impl GpuTexture {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
        pixels: &[u32],
        is_opaque: bool,
    ) -> Result<Self, String> {
        let w = width.max(1);
        let h = height.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Mango Compositor Texture"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = Arc::new(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mango Texture BindGroup"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        }));

        let mut gpu_tex = Self {
            texture,
            bind_group,
            width: w,
            height: h,
            version: 0,
        };
        gpu_tex.upload_pixels(queue, pixels, is_opaque)?;
        Ok(gpu_tex)
    }

    fn upload_pixels(
        &mut self,
        queue: &wgpu::Queue,
        pixels: &[u32],
        is_opaque: bool,
    ) -> Result<(), String> {
        if self.width == 0 || self.height == 0 {
            return Err("cannot upload a zero-sized GPU texture".to_string());
        }
        if pixels.is_empty() {
            return Err("cannot upload an empty GPU texture buffer".to_string());
        }
        let expected_len = (self.width as usize) * (self.height as usize);
        if pixels.len() < expected_len {
            return Err(format!(
                "GPU texture pixel buffer is too short: expected {expected_len}, got {}",
                pixels.len()
            ));
        }

        let mut bytes = PIXEL_UPLOAD_BUFFER
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .map_err(|_| "GPU upload staging buffer poisoned".to_string())?;
        bytes.resize(expected_len * 4, 0);

        if is_opaque {
            // Fast path for 100% opaque page tiles
            for (chunk, &px) in bytes
                .as_mut_slice()
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(pixels.iter())
            {
                chunk[0] = ((px >> 16) & 0xFF) as u8;
                chunk[1] = ((px >> 8) & 0xFF) as u8;
                chunk[2] = (px & 0xFF) as u8;
                chunk[3] = 255;
            }
        } else {
            // For chrome UI, overlay, and promoted layers with alpha
            for (chunk, &px) in bytes
                .as_mut_slice()
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(pixels.iter())
            {
                let a = ((px >> 24) & 0xFF) as u8;
                let alpha = if a != 0 {
                    a
                } else if px & 0x00FFFFFF != 0 {
                    255
                } else {
                    0
                };

                chunk[0] = ((px >> 16) & 0xFF) as u8;
                chunk[1] = ((px >> 8) & 0xFF) as u8;
                chunk[2] = (px & 0xFF) as u8;
                chunk[3] = alpha;
            }
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 4),
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }
}

struct QuadDraw {
    bind_group: Arc<wgpu::BindGroup>,
    vertices: [CompositorVertex; 6],
}

/// The hardware-accelerated GPU compositor powered by `wgpu`.
pub struct GpuCompositor {
    tile_textures: HashMap<TileCoord, GpuTexture>,
    promoted_textures: HashMap<u64, GpuTexture>,
    chrome_texture: Option<GpuTexture>,
    overlay_texture: Option<GpuTexture>,
    frame_texture: Option<GpuTexture>,
    frame_pixels: Vec<u32>,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_buffer_capacity: usize,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    queue: wgpu::Queue,
    device: wgpu::Device,
    pub width: u32,
    pub height: u32,
}

impl Drop for GpuCompositor {
    fn drop(&mut self) {
        self.tile_textures.clear();
        self.promoted_textures.clear();
        self.chrome_texture = None;
        self.overlay_texture = None;
        self.frame_texture = None;
        self.frame_pixels.clear();
        self.vertex_buffer = None;
    }
}

impl GpuCompositor {
    /// Attempts to initialize the hardware GPU compositor for the given window.
    pub fn new(window: Arc<Window>) -> Result<Self, String> {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::default();

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("Failed to create wgpu surface: {e}"))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| format!("Failed to find a compatible GPU adapter: {e}"))?;

        let info = adapter.get_info();
        log::info!(
            "wgpu adapter selected: {} ({:?}) via {:?}",
            info.name,
            info.device_type,
            info.backend
        );

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Mango GPU Device"),
            ..Default::default()
        }))
        .map_err(|e| format!("Failed to request wgpu device: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        let texture_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: texture_format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let shader_src = include_str!("shaders/compositor.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Mango Compositor Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Mango Texture BindGroup Layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mango Compositor Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            ..Default::default()
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Mango Compositor Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<CompositorVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32,
                        },
                    ],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Mango Compositor Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            tile_textures: HashMap::new(),
            promoted_textures: HashMap::new(),
            chrome_texture: None,
            overlay_texture: None,
            frame_texture: None,
            frame_pixels: Vec::new(),
            vertex_buffer: None,
            vertex_buffer_capacity: 0,
            pipeline,
            bind_group_layout,
            sampler,
            surface,
            config,
            queue,
            device,
            width,
            height,
        })
    }

    /// Resizes the GPU swapchain surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        if w == self.width && h == self.height {
            return;
        }
        self.width = w;
        self.height = h;
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
    }

    /// Composites the layer tree and presents to the window surface.
    pub fn render(&mut self, layers: &LayerTree, bg_color: Color) -> Result<(), String> {
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                    other => {
                        return Err(format!(
                            "Surface acquire failed after reconfigure: {other:?}"
                        ));
                    }
                }
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("Surface validation error".to_string());
            }
        };

        let clear_color = wgpu::Color {
            r: bg_color.r as f64 / 255.0,
            g: bg_color.g as f64 / 255.0,
            b: bg_color.b as f64 / 255.0,
            a: 1.0,
        };

        let w_f32 = self.width as f32;
        let h_f32 = self.height as f32;
        let mut quads: Vec<QuadDraw> = Vec::new();

        // 1. Prepare Page Layer Tiles
        let page = &layers.page_layer;
        let page_clip = page.bounds;

        if let Some(grid) = &page.tile_grid {
            let scroll_x = page.scroll_offset.0;
            let scroll_y = page.scroll_offset.1;

            let visible_viewport =
                Rect::new(scroll_x, scroll_y, page_clip.width(), page_clip.height());

            let visible_coords = grid.visible_tile_coords(visible_viewport, 0);
            let visible_set: std::collections::HashSet<TileCoord> =
                visible_coords.iter().copied().collect();
            self.tile_textures
                .retain(|coord, _| visible_set.contains(coord));

            for coord in visible_coords {
                if let Some(tile) = grid.get_tile(coord) {
                    // Skip tiles that have never completed a raster pass to prevent
                    // flashing black or blank placeholder rects over the clear color.
                    if tile.version == 0 && tile.dirty {
                        continue;
                    }

                    let layout = &self.bind_group_layout;
                    let sampler = &self.sampler;
                    let gpu_tex = match self.tile_textures.entry(coord) {
                        std::collections::hash_map::Entry::Occupied(entry) => {
                            let tex = entry.into_mut();
                            if tex.width != tile.width || tex.height != tile.height {
                                *tex = GpuTexture::new(
                                    &self.device,
                                    &self.queue,
                                    layout,
                                    sampler,
                                    tile.width,
                                    tile.height,
                                    &tile.pixels,
                                    true,
                                )?;
                            } else if tex.version != tile.version {
                                tex.upload_pixels(&self.queue, &tile.pixels, true)?;
                                tex.version = tile.version;
                            }
                            tex
                        }
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            let tex = GpuTexture::new(
                                &self.device,
                                &self.queue,
                                layout,
                                sampler,
                                tile.width,
                                tile.height,
                                &tile.pixels,
                                true,
                            )?;
                            entry.insert(tex)
                        }
                    };

                    let tile_rect_x = (tile.rect.x() - scroll_x + page_clip.x()).round();
                    let tile_rect_y = (tile.rect.y() - scroll_y + page_clip.y()).round();
                    let tile_w = tile.width as f32;
                    let tile_h = tile.height as f32;

                    // Integer-aligned clamp to page viewport bounds to avoid bleeding into toolbar
                    let clamp_x0 = tile_rect_x.max(page_clip.x());
                    let clamp_y0 = tile_rect_y.max(page_clip.y());
                    let clamp_x1 = (tile_rect_x + tile_w).min(page_clip.right());
                    let clamp_y1 = (tile_rect_y + tile_h).min(page_clip.bottom());

                    if clamp_x1 > clamp_x0 && clamp_y1 > clamp_y0 {
                        let u0 = (clamp_x0 - tile_rect_x) / tile_w;
                        let u1 = (clamp_x1 - tile_rect_x) / tile_w;
                        let v0 = (clamp_y0 - tile_rect_y) / tile_h;
                        let v1 = (clamp_y1 - tile_rect_y) / tile_h;

                        let ndc_x0 = (clamp_x0 / w_f32) * 2.0 - 1.0;
                        let ndc_y0 = 1.0 - (clamp_y0 / h_f32) * 2.0;
                        let ndc_x1 = (clamp_x1 / w_f32) * 2.0 - 1.0;
                        let ndc_y1 = 1.0 - (clamp_y1 / h_f32) * 2.0;

                        let verts =
                            make_quad_vertices(ndc_x0, ndc_y0, ndc_x1, ndc_y1, u0, v0, u1, v1, 1.0);
                        quads.push(QuadDraw {
                            bind_group: gpu_tex.bind_group.clone(),
                            vertices: verts,
                        });
                    }
                }
            }
        }

        // 2. Prepare Promoted Layers (e.g. will-change: transform / fixed positioning)
        let active_promoted: std::collections::HashSet<u64> = layers
            .promoted_layers
            .iter()
            .map(|layer| layer.id)
            .collect();
        self.promoted_textures
            .retain(|id, _| active_promoted.contains(id));
        for layer in &layers.promoted_layers {
            if let Some(pixels) = &layer.pixels {
                let lw = layer.bounds.width().max(1.0) as u32;
                let lh = layer.bounds.height().max(1.0) as u32;
                let layout = &self.bind_group_layout;
                let sampler = &self.sampler;
                let gpu_tex = match self.promoted_textures.entry(layer.id) {
                    std::collections::hash_map::Entry::Occupied(entry) => {
                        let tex = entry.into_mut();
                        if tex.width != lw || tex.height != lh {
                            *tex = GpuTexture::new(
                                &self.device,
                                &self.queue,
                                layout,
                                sampler,
                                lw,
                                lh,
                                pixels,
                                false,
                            )?;
                        } else if tex.version != layer.version {
                            tex.upload_pixels(&self.queue, pixels, false)?;
                            tex.version = layer.version;
                        }
                        tex
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let tex = GpuTexture::new(
                            &self.device,
                            &self.queue,
                            layout,
                            sampler,
                            lw,
                            lh,
                            pixels,
                            false,
                        )?;
                        entry.insert(tex)
                    }
                };

                let lx = layer.bounds.x().round();
                let ly = layer.bounds.y().round();
                let lw_f = layer.bounds.width().round();
                let lh_f = layer.bounds.height().round();

                let clamp_x0 = lx.max(page_clip.x());
                let clamp_y0 = ly.max(page_clip.y());
                let clamp_x1 = (lx + lw_f).min(page_clip.right());
                let clamp_y1 = (ly + lh_f).min(page_clip.bottom());

                if clamp_x1 > clamp_x0 && clamp_y1 > clamp_y0 {
                    let u0 = (clamp_x0 - lx) / lw_f;
                    let u1 = (clamp_x1 - lx) / lw_f;
                    let v0 = (clamp_y0 - ly) / lh_f;
                    let v1 = (clamp_y1 - ly) / lh_f;

                    let ndc_x0 = (clamp_x0 / w_f32) * 2.0 - 1.0;
                    let ndc_y0 = 1.0 - (clamp_y0 / h_f32) * 2.0;
                    let ndc_x1 = (clamp_x1 / w_f32) * 2.0 - 1.0;
                    let ndc_y1 = 1.0 - (clamp_y1 / h_f32) * 2.0;

                    let verts =
                        make_quad_vertices(ndc_x0, ndc_y0, ndc_x1, ndc_y1, u0, v0, u1, v1, 1.0);
                    quads.push(QuadDraw {
                        bind_group: gpu_tex.bind_group.clone(),
                        vertices: verts,
                    });
                }
            }
        }

        // 3. Prepare Chrome Layer (overlay with transparent background)
        let chrome = &layers.chrome_layer;
        if let Some(chrome_pixels) = &chrome.pixels {
            let cw = chrome.bounds.width() as u32;
            let ch = chrome.bounds.height() as u32;
            let layout = &self.bind_group_layout;
            let sampler = &self.sampler;

            if let Some(tex) = &mut self.chrome_texture {
                if tex.width != cw || tex.height != ch {
                    *tex = GpuTexture::new(
                        &self.device,
                        &self.queue,
                        layout,
                        sampler,
                        cw,
                        ch,
                        chrome_pixels,
                        false,
                    )?;
                } else if tex.version != chrome.version {
                    tex.upload_pixels(&self.queue, chrome_pixels, false)?;
                    tex.version = chrome.version;
                }
            } else {
                self.chrome_texture = Some(GpuTexture::new(
                    &self.device,
                    &self.queue,
                    layout,
                    sampler,
                    cw,
                    ch,
                    chrome_pixels,
                    false,
                )?);
            }

            if let Some(tex) = &self.chrome_texture {
                let cx0 = chrome.bounds.x().round();
                let cy0 = chrome.bounds.y().round();
                let cx1 = cx0 + chrome.bounds.width().round();
                let cy1 = cy0 + chrome.bounds.height().round();

                let ndc_x0 = (cx0 / w_f32) * 2.0 - 1.0;
                let ndc_y0 = 1.0 - (cy0 / h_f32) * 2.0;
                let ndc_x1 = (cx1 / w_f32) * 2.0 - 1.0;
                let ndc_y1 = 1.0 - (cy1 / h_f32) * 2.0;

                let verts =
                    make_quad_vertices(ndc_x0, ndc_y0, ndc_x1, ndc_y1, 0.0, 0.0, 1.0, 1.0, 1.0);
                quads.push(QuadDraw {
                    bind_group: tex.bind_group.clone(),
                    vertices: verts,
                });
            }
        }

        // 4. Prepare Overlay Layer (if present)
        if let Some(overlay) = &layers.overlay_layer
            && let Some(overlay_pixels) = &overlay.pixels
        {
            let ow = overlay.bounds.width() as u32;
            let oh = overlay.bounds.height() as u32;
            let layout = &self.bind_group_layout;
            let sampler = &self.sampler;

            if let Some(tex) = &mut self.overlay_texture {
                if tex.width != ow || tex.height != oh {
                    *tex = GpuTexture::new(
                        &self.device,
                        &self.queue,
                        layout,
                        sampler,
                        ow,
                        oh,
                        overlay_pixels,
                        false,
                    )?;
                } else if tex.version != overlay.version {
                    tex.upload_pixels(&self.queue, overlay_pixels, false)?;
                    tex.version = overlay.version;
                }
            } else {
                self.overlay_texture = Some(GpuTexture::new(
                    &self.device,
                    &self.queue,
                    layout,
                    sampler,
                    ow,
                    oh,
                    overlay_pixels,
                    false,
                )?);
            }

            if let Some(tex) = &self.overlay_texture {
                let ox0 = overlay.bounds.x().round();
                let oy0 = overlay.bounds.y().round();
                let ox1 = ox0 + overlay.bounds.width().round();
                let oy1 = oy0 + overlay.bounds.height().round();

                let ndc_x0 = (ox0 / w_f32) * 2.0 - 1.0;
                let ndc_y0 = 1.0 - (oy0 / h_f32) * 2.0;
                let ndc_x1 = (ox1 / w_f32) * 2.0 - 1.0;
                let ndc_y1 = 1.0 - (oy1 / h_f32) * 2.0;

                let verts =
                    make_quad_vertices(ndc_x0, ndc_y0, ndc_x1, ndc_y1, 0.0, 0.0, 1.0, 1.0, 1.0);
                quads.push(QuadDraw {
                    bind_group: tex.bind_group.clone(),
                    vertices: verts,
                });
            }
        }

        // 5. Build and execute Render Pass
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Mango Compositor Encoder"),
            });

        if !quads.is_empty() {
            let total_vertices = quads.len() * 6;
            if self.vertex_buffer.is_none() || self.vertex_buffer_capacity < total_vertices {
                let new_cap = total_vertices.max(256);
                self.vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Mango Compositor Vertex Buffer"),
                    size: (new_cap * std::mem::size_of::<CompositorVertex>())
                        as wgpu::BufferAddress,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.vertex_buffer_capacity = new_cap;
            }

            let mut all_verts = Vec::with_capacity(total_vertices);
            for q in &quads {
                all_verts.extend_from_slice(&q.vertices);
            }
            if let Some(vb) = &self.vertex_buffer {
                self.queue
                    .write_buffer(vb, 0, bytemuck::cast_slice(&all_verts));
            }

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Mango Compositor Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear_color),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });

                pass.set_pipeline(&self.pipeline);
                if let Some(vb) = &self.vertex_buffer {
                    pass.set_vertex_buffer(0, vb.slice(..));
                }

                for (i, q) in quads.iter().enumerate() {
                    pass.set_bind_group(0, &*q.bind_group, &[]);
                    let start = (i * 6) as u32;
                    let end = start + 6;
                    pass.draw(start..end, 0..1);
                }
            }
        } else {
            // Just clear the screen
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Mango Compositor Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);

        Ok(())
    }

    /// Rasterizes a full display list and presents it directly to the window surface via the GPU pipeline.
    pub fn render_display_list(
        &mut self,
        dl: &mango_render::display_list::DisplayList,
        bg_color: Color,
    ) -> Result<(), String> {
        let w = self.width;
        let h = self.height;
        if w == 0 || h == 0 {
            return Ok(());
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                    other => {
                        return Err(format!(
                            "Surface acquire failed after reconfigure: {other:?}"
                        ));
                    }
                }
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("Surface validation error".to_string());
            }
        };

        self.frame_pixels.resize((w as usize) * (h as usize), 0);
        self.frame_pixels.fill(bg_color.to_rgb_u32());
        mango_render::painter::paint(dl, &mut self.frame_pixels, w, h);

        let layout = &self.bind_group_layout;
        let sampler = &self.sampler;
        if let Some(tex) = &mut self.frame_texture {
            if tex.width != w || tex.height != h {
                *tex = GpuTexture::new(
                    &self.device,
                    &self.queue,
                    layout,
                    sampler,
                    w,
                    h,
                    &self.frame_pixels,
                    true,
                )?;
            } else {
                tex.upload_pixels(&self.queue, &self.frame_pixels, true)?;
            }
        } else {
            self.frame_texture = Some(GpuTexture::new(
                &self.device,
                &self.queue,
                layout,
                sampler,
                w,
                h,
                &self.frame_pixels,
                true,
            )?);
        }

        let clear_color = wgpu::Color {
            r: bg_color.r as f64 / 255.0,
            g: bg_color.g as f64 / 255.0,
            b: bg_color.b as f64 / 255.0,
            a: 1.0,
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Mango Display List Encoder"),
            });

        if let Some(tex) = &self.frame_texture {
            let verts = make_quad_vertices(-1.0, 1.0, 1.0, -1.0, 0.0, 0.0, 1.0, 1.0, 1.0);
            if self.vertex_buffer.is_none() || self.vertex_buffer_capacity < 6 {
                self.vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Mango Compositor Vertex Buffer"),
                    size: (6 * std::mem::size_of::<CompositorVertex>()) as wgpu::BufferAddress,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.vertex_buffer_capacity = 6;
            }

            if let Some(vb) = &self.vertex_buffer {
                self.queue.write_buffer(vb, 0, bytemuck::cast_slice(&verts));
            }

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Mango Display List Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear_color),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });

                pass.set_pipeline(&self.pipeline);
                if let Some(vb) = &self.vertex_buffer {
                    pass.set_vertex_buffer(0, vb.slice(..));
                }
                pass.set_bind_group(0, &*tex.bind_group, &[]);
                pass.draw(0..6, 0..1);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);

        Ok(())
    }
}

/// Generates 6 vertices forming two triangles for an NDC-space rectangle and UV bounds.
fn make_quad_vertices(
    ndc_x0: f32,
    ndc_y0: f32,
    ndc_x1: f32,
    ndc_y1: f32,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    opacity: f32,
) -> [CompositorVertex; 6] {
    [
        // Triangle 1
        CompositorVertex::new([ndc_x0, ndc_y0], [u0, v0], opacity),
        CompositorVertex::new([ndc_x0, ndc_y1], [u0, v1], opacity),
        CompositorVertex::new([ndc_x1, ndc_y1], [u1, v1], opacity),
        // Triangle 2
        CompositorVertex::new([ndc_x0, ndc_y0], [u0, v0], opacity),
        CompositorVertex::new([ndc_x1, ndc_y1], [u1, v1], opacity),
        CompositorVertex::new([ndc_x1, ndc_y0], [u1, v0], opacity),
    ]
}
