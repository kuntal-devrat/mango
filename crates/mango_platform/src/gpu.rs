//! Hardware-accelerated GPU Compositor powered by `wgpu`.
//!
//! Merges rasterized UI layers and page tiles directly in GPU memory using
//! textured quad primitives. Enables 60+ FPS butter-smooth scrolling and
//! hardware-accelerated presentation with minimal CPU overhead.

use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use mango_core::{Color, Rect};
use mango_render::layer::LayerTree;
use mango_render::tile::TileCoord;
use winit::window::Window;

/// Vertex structure for textured quad compositing.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadVertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
}

impl QuadVertex {
    pub const fn new(position: [f32; 2], tex_coords: [f32; 2]) -> Self {
        Self {
            position,
            tex_coords,
        }
    }

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// An allocated GPU 2D texture and its associated bind group.
struct GpuTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    version: u64,
}

impl GpuTexture {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sampler: &wgpu::Sampler,
        layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        pixels: &[u32],
        is_opaque: bool,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Mango Compositor Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mango Texture Bind Group"),
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
        });

        let mut gpu_tex = Self {
            texture,
            bind_group,
            width,
            height,
            version: 0,
        };
        gpu_tex.upload_pixels(queue, pixels, is_opaque);
        gpu_tex
    }

    fn upload_pixels(&mut self, queue: &wgpu::Queue, pixels: &[u32], is_opaque: bool) {
        if pixels.is_empty() || self.width == 0 || self.height == 0 {
            return;
        }
        let expected_len = (self.width as usize) * (self.height as usize);
        if pixels.len() < expected_len {
            return;
        }

        let mut rgba_bytes = Vec::with_capacity(pixels.len() * 4);
        if is_opaque {
            // For page tiles: every pixel is 100% opaque.
            // Even when px == 0 (black text or background), alpha MUST be 255.
            for &px in pixels {
                let r = ((px >> 16) & 0xFF) as u8;
                let g = ((px >> 8) & 0xFF) as u8;
                let b = (px & 0xFF) as u8;
                rgba_bytes.extend_from_slice(&[r, g, b, 255]);
            }
        } else {
            // For chrome UI and overlay: 0x00000000 is transparent (page content shines through).
            // Any pixel drawn by chrome UI has alpha = 255 (or top-byte alpha if set).
            for &px in pixels {
                let r = ((px >> 16) & 0xFF) as u8;
                let g = ((px >> 8) & 0xFF) as u8;
                let b = (px & 0xFF) as u8;
                let a = ((px >> 24) & 0xFF) as u8;
                let alpha = if a != 0 {
                    a
                } else if px != 0 {
                    255
                } else {
                    0
                };
                rgba_bytes.extend_from_slice(&[r, g, b, alpha]);
            }
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.width.max(1)),
                rows_per_image: Some(self.height.max(1)),
            },
            wgpu::Extent3d {
                width: self.width.max(1),
                height: self.height.max(1),
                depth_or_array_layers: 1,
            },
        );
    }
}

/// The hardware-accelerated GPU compositor.
pub struct GpuCompositor {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    vertex_buffer: wgpu::Buffer,
    texture_format: wgpu::TextureFormat,
    tile_textures: HashMap<TileCoord, GpuTexture>,
    chrome_texture: Option<GpuTexture>,
    overlay_texture: Option<GpuTexture>,
    pub width: u32,
    pub height: u32,
}

impl GpuCompositor {
    /// Attempts to initialize the hardware GPU compositor for the given window.
    pub fn new(window: Arc<Window>) -> Result<Self, String> {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(window.clone()),
        ));

        let surface = instance
            .create_surface(window)
            .map_err(|e| format!("failed to create wgpu surface: {e}"))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|e| format!("no compatible GPU adapter found: {e}"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Mango GPU Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| format!("failed to create wgpu device: {e}"))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        // Ensure texture color space exactly matches swapchain target format
        let texture_format = if surface_format.is_srgb() {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };

        // Prefer Opaque window presentation so desktop compositor never renders window as translucent
        let alpha_mode = if surface_caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else if surface_caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Auto) {
            wgpu::CompositeAlphaMode::Auto
        } else {
            surface_caps.alpha_modes[0]
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Srgb,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Mango Compositor Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/compositor.wgsl").into()),
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Mango Compositor Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
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
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Mango Compositor Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(QuadVertex::desc())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // Use Nearest filtering for pixel-crisp 1:1 UI and font rasterization
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // Pre-allocate a reusable vertex buffer for up to 1024 textured quads
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mango Quad Vertex Buffer"),
            size: (std::mem::size_of::<QuadVertex>() * 6 * 1024) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            surface,
            config,
            render_pipeline,
            sampler,
            bind_group_layout,
            vertex_buffer,
            texture_format,
            tile_textures: HashMap::new(),
            chrome_texture: None,
            overlay_texture: None,
            width,
            height,
        })
    }

    /// Resizes the GPU swapchain surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
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

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(tex) | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => tex,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            _ => return Ok(()),
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Mango Render Encoder"),
            });

        // Background clear color
        let clear_color = wgpu::Color {
            r: bg_color.r as f64 / 255.0,
            g: bg_color.g as f64 / 255.0,
            b: bg_color.b as f64 / 255.0,
            a: 1.0,
        };

        let mut tile_quads: Vec<([QuadVertex; 6], wgpu::BindGroup)> = Vec::new();
        let mut chrome_quads: Vec<([QuadVertex; 6], wgpu::BindGroup)> = Vec::new();

        let w_f32 = self.width as f32;
        let h_f32 = self.height as f32;

        // 1. Prepare Page Layer Tiles
        let page = &layers.page_layer;
        let page_clip = page.bounds;
        let clip_x = page_clip.x().round().max(0.0) as u32;
        let clip_y = page_clip.y().round().max(0.0) as u32;
        let clip_w = (page_clip.width().round() as u32).min(self.width.saturating_sub(clip_x));
        let clip_h = (page_clip.height().round() as u32).min(self.height.saturating_sub(clip_y));

        if let Some(grid) = &page.tile_grid {
            let scroll_x = page.scroll_offset.0;
            let scroll_y = page.scroll_offset.1;

            let visible_viewport = Rect::new(
                scroll_x,
                scroll_y,
                page_clip.width(),
                page_clip.height(),
            );

            let visible_coords = grid.visible_tile_coords(visible_viewport, 0);
            let visible_set: std::collections::HashSet<TileCoord> =
                visible_coords.iter().copied().collect();
            self.tile_textures.retain(|coord, _| visible_set.contains(coord));

            for coord in visible_coords {
                if let Some(tile) = grid.get_tile(coord) {
                    let tf = self.texture_format;
                    let gpu_tex = self.tile_textures.entry(coord).or_insert_with(|| {
                        GpuTexture::new(
                            &self.device,
                            &self.queue,
                            &self.sampler,
                            &self.bind_group_layout,
                            tf,
                            tile.width,
                            tile.height,
                            &tile.pixels,
                            true,
                        )
                    });

                    if gpu_tex.width != tile.width || gpu_tex.height != tile.height {
                        *gpu_tex = GpuTexture::new(
                            &self.device,
                            &self.queue,
                            &self.sampler,
                            &self.bind_group_layout,
                            tf,
                            tile.width,
                            tile.height,
                            &tile.pixels,
                            true,
                        );
                    } else if gpu_tex.version != tile.version {
                        gpu_tex.upload_pixels(&self.queue, &tile.pixels, true);
                        gpu_tex.version = tile.version;
                    }

                    // Integer-aligned screen quad calculation
                    let dest_x = (tile.rect.x() - scroll_x + page_clip.x()).round();
                    let dest_y = (tile.rect.y() - scroll_y + page_clip.y()).round();
                    let dest_w = tile.width as f32;
                    let dest_h = tile.height as f32;

                    let verts = make_quad_vertices(
                        dest_x, dest_y, dest_w, dest_h, w_f32, h_f32,
                    );
                    tile_quads.push((verts, gpu_tex.bind_group.clone()));
                }
            }
        }

        // 2. Prepare Chrome Layer (overlay with transparent background)
        let chrome = &layers.chrome_layer;
        if let Some(chrome_pixels) = &chrome.pixels {
            let cw = chrome.bounds.width() as u32;
            let ch = chrome.bounds.height() as u32;
            let tf = self.texture_format;

            if let Some(tex) = &mut self.chrome_texture {
                if tex.width != cw || tex.height != ch {
                    *tex = GpuTexture::new(
                        &self.device,
                        &self.queue,
                        &self.sampler,
                        &self.bind_group_layout,
                        tf,
                        cw,
                        ch,
                        chrome_pixels,
                        false,
                    );
                } else if tex.version != chrome.version {
                    tex.upload_pixels(&self.queue, chrome_pixels, false);
                    tex.version = chrome.version;
                }
            } else {
                self.chrome_texture = Some(GpuTexture::new(
                    &self.device,
                    &self.queue,
                    &self.sampler,
                    &self.bind_group_layout,
                    tf,
                    cw,
                    ch,
                    chrome_pixels,
                    false,
                ));
            }

            if let Some(tex) = &self.chrome_texture {
                let verts = make_quad_vertices(
                    chrome.bounds.x().round(),
                    chrome.bounds.y().round(),
                    chrome.bounds.width().round(),
                    chrome.bounds.height().round(),
                    w_f32,
                    h_f32,
                );
                chrome_quads.push((verts, tex.bind_group.clone()));
            }
        }

        // 3. Prepare Overlay Layer (if present)
        if let Some(overlay) = &layers.overlay_layer
            && let Some(overlay_pixels) = &overlay.pixels
        {
            let ow = overlay.bounds.width() as u32;
            let oh = overlay.bounds.height() as u32;
            let tf = self.texture_format;

            if let Some(tex) = &mut self.overlay_texture {
                if tex.width != ow || tex.height != oh {
                    *tex = GpuTexture::new(
                        &self.device,
                        &self.queue,
                        &self.sampler,
                        &self.bind_group_layout,
                        tf,
                        ow,
                        oh,
                        overlay_pixels,
                        false,
                    );
                } else if tex.version != overlay.version {
                    tex.upload_pixels(&self.queue, overlay_pixels, false);
                    tex.version = overlay.version;
                }
            } else {
                self.overlay_texture = Some(GpuTexture::new(
                    &self.device,
                    &self.queue,
                    &self.sampler,
                    &self.bind_group_layout,
                    tf,
                    ow,
                    oh,
                    overlay_pixels,
                    false,
                ));
            }

            if let Some(tex) = &self.overlay_texture {
                let verts = make_quad_vertices(
                    overlay.bounds.x().round(),
                    overlay.bounds.y().round(),
                    overlay.bounds.width().round(),
                    overlay.bounds.height().round(),
                    w_f32,
                    h_f32,
                );
                chrome_quads.push((verts, tex.bind_group.clone()));
            }
        }

        // 4. Execute Render Pass
        let total_quads = tile_quads.len() + chrome_quads.len();
        if total_quads == 0 {
            self.queue.submit(std::iter::once(encoder.finish()));
            self.queue.present(output);
            return Ok(());
        }

        let mut all_vertices: Vec<QuadVertex> = Vec::with_capacity(total_quads * 6);
        for (verts, _) in &tile_quads {
            all_vertices.extend_from_slice(verts);
        }
        for (verts, _) in &chrome_quads {
            all_vertices.extend_from_slice(verts);
        }

        let required_size =
            (std::mem::size_of::<QuadVertex>() * all_vertices.len()) as wgpu::BufferAddress;
        if self.vertex_buffer.size() < required_size {
            self.vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Mango Quad Vertex Buffer"),
                size: required_size.max(self.vertex_buffer.size() * 2),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&all_vertices),
        );

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Mango Render Pass"),
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

        render_pass.set_pipeline(&self.render_pipeline);

        let quad_stride = (std::mem::size_of::<QuadVertex>() * 6) as wgpu::BufferAddress;
        let mut quad_idx = 0;

        // Draw page tiles restricted by scissor rect to avoid bleeding into chrome/status bar
        if clip_w > 0 && clip_h > 0 {
            render_pass.set_scissor_rect(clip_x, clip_y, clip_w, clip_h);
        }
        for (_, bind_group) in &tile_quads {
            let offset = (quad_idx as wgpu::BufferAddress) * quad_stride;
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.set_vertex_buffer(
                0,
                self.vertex_buffer.slice(offset..offset + quad_stride),
            );
            render_pass.draw(0..6, 0..1);
            quad_idx += 1;
        }

        // Reset scissor rect to full window for Chrome UI and overlay layers
        render_pass.set_scissor_rect(0, 0, self.width, self.height);
        for (_, bind_group) in &chrome_quads {
            let offset = (quad_idx as wgpu::BufferAddress) * quad_stride;
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.set_vertex_buffer(
                0,
                self.vertex_buffer.slice(offset..offset + quad_stride),
            );
            render_pass.draw(0..6, 0..1);
            quad_idx += 1;
        }

        drop(render_pass);

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(output);

        Ok(())
    }
}

/// Helper converting screen pixel coordinates into normalized device coordinates (NDC: [-1.0, 1.0]).
fn make_quad_vertices(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    screen_w: f32,
    screen_h: f32,
) -> [QuadVertex; 6] {
    let ndc_x0 = (x / screen_w) * 2.0 - 1.0;
    let ndc_y0 = 1.0 - (y / screen_h) * 2.0;
    let ndc_x1 = ((x + w) / screen_w) * 2.0 - 1.0;
    let ndc_y1 = 1.0 - ((y + h) / screen_h) * 2.0;

    [
        QuadVertex::new([ndc_x0, ndc_y0], [0.0, 0.0]),
        QuadVertex::new([ndc_x0, ndc_y1], [0.0, 1.0]),
        QuadVertex::new([ndc_x1, ndc_y1], [1.0, 1.0]),
        QuadVertex::new([ndc_x0, ndc_y0], [0.0, 0.0]),
        QuadVertex::new([ndc_x1, ndc_y1], [1.0, 1.0]),
        QuadVertex::new([ndc_x1, ndc_y0], [1.0, 0.0]),
    ]
}
