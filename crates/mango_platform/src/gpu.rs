//! Hardware-accelerated GPU Compositor powered by `lgpu` (Lean GPU).
//!
//! Merges rasterized UI layers and page tiles directly in GPU memory using
//! modern Vulkan 1.3 dynamic rendering and bindless descriptor indexing.
//! Enables 60+ FPS butter-smooth scrolling and zero-copy presentation
//! with sub-second compile times and minimal binary footprint.

use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use mango_core::{Color, Rect};
use mango_render::layer::LayerTree;
use mango_render::tile::TileCoord;
use winit::window::Window;

/// Vertex structure kept for backwards compatibility and geometry definitions.
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
}

/// Push constant payload passed to the bindless quad shader (48 bytes total).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadPush {
    pub rect: [f32; 4],
    pub uv_rect: [f32; 4],
    pub tex_index: u32,
    pub sampler_index: u32,
    pub flags: u32,
    pub opacity: f32,
}

/// An allocated GPU 2D texture managed within lgpu's global bindless heap.
struct GpuTexture {
    texture: lgpu::Texture,
    width: u32,
    height: u32,
    version: u64,
}

impl GpuTexture {
    fn new(
        device: &lgpu::Device,
        queue: &lgpu::Queue,
        format: lgpu::TextureFormat,
        width: u32,
        height: u32,
        pixels: &[u32],
        is_opaque: bool,
    ) -> Result<Self, String> {
        let texture = device
            .create_texture(&lgpu::TextureDesc {
                label: Some("Mango Compositor Texture"),
                width: width.max(1),
                height: height.max(1),
                depth: 1,
                mip_levels: 1,
                sample_count: 1,
                format,
                usage: lgpu::TextureUsage::TEXTURE_BINDING | lgpu::TextureUsage::COPY_DST,
            })
            .map_err(|e| format!("Failed to create lgpu texture: {e}"))?;

        let mut gpu_tex = Self {
            texture,
            width,
            height,
            version: 0,
        };
        gpu_tex.upload_pixels(queue, format, pixels, is_opaque)?;
        Ok(gpu_tex)
    }

    fn upload_pixels(
        &mut self,
        queue: &lgpu::Queue,
        format: lgpu::TextureFormat,
        pixels: &[u32],
        is_opaque: bool,
    ) -> Result<(), String> {
        if pixels.is_empty() || self.width == 0 || self.height == 0 {
            return Ok(());
        }
        let expected_len = (self.width as usize) * (self.height as usize);
        if pixels.len() < expected_len {
            return Ok(());
        }

        let is_bgra = matches!(
            format,
            lgpu::TextureFormat::Bgra8Unorm | lgpu::TextureFormat::Bgra8Srgb
        );

        let mut bytes = vec![0u8; expected_len * 4];
        if is_opaque {
            // For page tiles: every pixel is 100% opaque.
            for (chunk, &px) in bytes.chunks_exact_mut(4).zip(pixels.iter()) {
                if is_bgra {
                    chunk[0] = (px & 0xFF) as u8;
                    chunk[1] = ((px >> 8) & 0xFF) as u8;
                    chunk[2] = ((px >> 16) & 0xFF) as u8;
                    chunk[3] = 255;
                } else {
                    chunk[0] = ((px >> 16) & 0xFF) as u8;
                    chunk[1] = ((px >> 8) & 0xFF) as u8;
                    chunk[2] = (px & 0xFF) as u8;
                    chunk[3] = 255;
                }
            }
        } else {
            // For chrome UI, overlay, and promoted layers:
            for (chunk, &px) in bytes.chunks_exact_mut(4).zip(pixels.iter()) {
                let a = ((px >> 24) & 0xFF) as u8;
                let alpha = if a != 0 {
                    a
                } else if px & 0x00FFFFFF != 0 {
                    255
                } else {
                    0
                };

                if is_bgra {
                    chunk[0] = (px & 0xFF) as u8;
                    chunk[1] = ((px >> 8) & 0xFF) as u8;
                    chunk[2] = ((px >> 16) & 0xFF) as u8;
                    chunk[3] = alpha;
                } else {
                    chunk[0] = ((px >> 16) & 0xFF) as u8;
                    chunk[1] = ((px >> 8) & 0xFF) as u8;
                    chunk[2] = (px & 0xFF) as u8;
                    chunk[3] = alpha;
                }
            }
        }

        queue
            .write_texture(&self.texture, &bytes)
            .map_err(|e| format!("write_texture upload failed: {e}"))?;

        Ok(())
    }
}

/// The hardware-accelerated GPU compositor powered by `lgpu`.
pub struct GpuCompositor {
    tile_textures: HashMap<TileCoord, GpuTexture>,
    promoted_textures: HashMap<usize, GpuTexture>,
    chrome_texture: Option<GpuTexture>,
    overlay_texture: Option<GpuTexture>,
    frame_texture: Option<GpuTexture>,
    pipeline: lgpu::GraphicsPipeline,
    sampler: lgpu::Sampler,
    surface: lgpu::Surface,
    config: lgpu::SurfaceConfiguration,
    texture_format: lgpu::TextureFormat,
    queue: lgpu::Queue,
    device: lgpu::Device,
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
    }
}

impl GpuCompositor {
    /// Attempts to initialize the hardware GPU compositor for the given window.
    pub fn new(window: Arc<Window>) -> Result<Self, String> {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance =
            lgpu::Instance::new().map_err(|e| format!("Failed to create lgpu instance: {e}"))?;
        let adapters = instance
            .enumerate_adapters()
            .map_err(|e| format!("Failed to enumerate GPU adapters: {e}"))?;
        if adapters.is_empty() {
            return Err("No compatible GPU adapter found".to_string());
        }

        let adapter = &adapters[0];
        let info = adapter.info();
        log::info!(
            "lgpu device selected: {} ({:?}) via {:?}",
            info.name,
            info.device_type,
            info.backend
        );

        let (device, queue) = adapter
            .request_device(&instance)
            .map_err(|e| format!("Failed to request lgpu device: {e}"))?;

        let mut surface = instance
            .create_surface(&*window)
            .map_err(|e| format!("Failed to create lgpu surface: {e}"))?;

        let texture_format = lgpu::TextureFormat::Bgra8Unorm;
        let config = lgpu::SurfaceConfiguration {
            usage: lgpu::TextureUsage::RENDER_ATTACHMENT,
            format: texture_format,
            width,
            height,
            present_mode: lgpu::PresentMode::Fifo,
        };
        surface
            .configure(&device, &config)
            .map_err(|e| format!("Failed to configure lgpu surface: {e}"))?;

        // Compile compositor WGSL shader into SPIR-V bytecode using naga
        let wgsl_src = include_str!("shaders/compositor.wgsl");
        let naga_module = naga::front::wgsl::parse_str(wgsl_src)
            .map_err(|e| format!("Failed to parse compositor WGSL: {e:?}"))?;

        let caps = naga::valid::Capabilities::all();
        let naga_info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), caps)
            .validate(&naga_module)
            .map_err(|e| format!("Failed to validate compositor shader: {e:?}"))?;

        let spv_options = naga::back::spv::Options {
            lang_version: (1, 3),
            ..Default::default()
        };
        let spv_words = naga::back::spv::write_vec(&naga_module, &naga_info, &spv_options, None)
            .map_err(|e| format!("Failed to translate WGSL to SPIR-V: {e:?}"))?;

        let shader = device
            .create_shader_spirv(&spv_words)
            .map_err(|e| format!("Failed to create SPIR-V shader module: {e}"))?;

        let pipeline_desc = lgpu::GraphicsPipelineDesc {
            label: Some("Mango Compositor Pipeline"),
            vs_entry: "vs_main",
            fs_entry: "fs_main",
            color_format: texture_format,
            depth_format: None,
            depth_test: false,
            depth_write: false,
            sample_count: 1,
            primitive: lgpu::PrimitiveState::default(),
            blend: Some(lgpu::BlendState::ALPHA_BLENDING),
        };
        let pipeline = device
            .create_graphics_pipeline_with_desc(&shader, &pipeline_desc)
            .map_err(|e| format!("Failed to create compositor graphics pipeline: {e}"))?;

        let sampler = device
            .create_sampler(&lgpu::SamplerDesc {
                label: Some("Mango Compositor Sampler"),
                address_mode_u: lgpu::AddressMode::ClampToEdge,
                address_mode_v: lgpu::AddressMode::ClampToEdge,
                address_mode_w: lgpu::AddressMode::ClampToEdge,
                mag_filter: lgpu::FilterMode::Nearest,
                min_filter: lgpu::FilterMode::Nearest,
                mipmap_filter: lgpu::FilterMode::Nearest,
            })
            .map_err(|e| format!("Failed to create compositor sampler: {e}"))?;

        Ok(Self {
            device,
            queue,
            surface,
            config,
            pipeline,
            sampler,
            texture_format,
            tile_textures: HashMap::new(),
            promoted_textures: HashMap::new(),
            chrome_texture: None,
            overlay_texture: None,
            frame_texture: None,
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
        if let Err(e) = self.surface.configure(&self.device, &self.config) {
            log::warn!("Failed to reconfigure surface on resize: {e}");
        }
    }

    /// Composites the layer tree and presents to the window surface.
    pub fn render(&mut self, layers: &LayerTree, bg_color: Color) -> Result<(), String> {
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                if let Err(e) = self.surface.configure(&self.device, &self.config) {
                    log::warn!("Surface reconfigure failed during frame acquire: {e}");
                    return Ok(());
                }
                match self.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(e) => {
                        log::warn!("lgpu get_current_texture failed: {e}");
                        return Ok(());
                    }
                }
            }
        };

        let clear_color = [
            bg_color.r as f32 / 255.0,
            bg_color.g as f32 / 255.0,
            bg_color.b as f32 / 255.0,
            1.0,
        ];

        let sampler_id = self.sampler.id().map(|id| id.0).unwrap_or(0);
        let w_f32 = self.width as f32;
        let h_f32 = self.height as f32;
        let mut quads: Vec<QuadPush> = Vec::new();

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
                    let tf = self.texture_format;
                    let gpu_tex = match self.tile_textures.entry(coord) {
                        std::collections::hash_map::Entry::Occupied(entry) => {
                            let tex = entry.into_mut();
                            if tex.width != tile.width || tex.height != tile.height {
                                *tex = GpuTexture::new(
                                    &self.device,
                                    &self.queue,
                                    tf,
                                    tile.width,
                                    tile.height,
                                    &tile.pixels,
                                    true,
                                )?;
                            } else if tex.version != tile.version {
                                tex.upload_pixels(&self.queue, tf, &tile.pixels, true)?;
                                tex.version = tile.version;
                            }
                            tex
                        }
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            let tex = GpuTexture::new(
                                &self.device,
                                &self.queue,
                                tf,
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

                        if let Some(tex_id) = gpu_tex.texture.id() {
                            quads.push(QuadPush {
                                rect: [ndc_x0, ndc_y0, ndc_x1, ndc_y1],
                                uv_rect: [u0, v0, u1, v1],
                                tex_index: tex_id.0,
                                sampler_index: sampler_id,
                                flags: 1,
                                opacity: 1.0,
                            });
                        }
                    }
                }
            }
        }

        // 2. Prepare Promoted Layers (e.g. will-change: transform / fixed positioning)
        let tf = self.texture_format;
        for (idx, layer) in layers.promoted_layers.iter().enumerate() {
            if let Some(pixels) = &layer.pixels {
                let lw = layer.bounds.width().max(1.0) as u32;
                let lh = layer.bounds.height().max(1.0) as u32;

                let gpu_tex = match self.promoted_textures.entry(idx) {
                    std::collections::hash_map::Entry::Occupied(entry) => {
                        let tex = entry.into_mut();
                        if tex.width != lw || tex.height != lh {
                            *tex = GpuTexture::new(
                                &self.device,
                                &self.queue,
                                tf,
                                lw,
                                lh,
                                pixels,
                                false,
                            )?;
                        } else if tex.version != layer.version {
                            tex.upload_pixels(&self.queue, tf, pixels, false)?;
                            tex.version = layer.version;
                        }
                        tex
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let tex =
                            GpuTexture::new(&self.device, &self.queue, tf, lw, lh, pixels, false)?;
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

                    if let Some(tex_id) = gpu_tex.texture.id() {
                        quads.push(QuadPush {
                            rect: [ndc_x0, ndc_y0, ndc_x1, ndc_y1],
                            uv_rect: [u0, v0, u1, v1],
                            tex_index: tex_id.0,
                            sampler_index: sampler_id,
                            flags: 0,
                            opacity: 1.0,
                        });
                    }
                }
            }
        }

        // 3. Prepare Chrome Layer (overlay with transparent background)
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
                        tf,
                        cw,
                        ch,
                        chrome_pixels,
                        false,
                    )?;
                } else if tex.version != chrome.version {
                    tex.upload_pixels(&self.queue, tf, chrome_pixels, false)?;
                    tex.version = chrome.version;
                }
            } else {
                self.chrome_texture = Some(GpuTexture::new(
                    &self.device,
                    &self.queue,
                    tf,
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

                if let Some(tex_id) = tex.texture.id() {
                    quads.push(QuadPush {
                        rect: [ndc_x0, ndc_y0, ndc_x1, ndc_y1],
                        uv_rect: [0.0, 0.0, 1.0, 1.0],
                        tex_index: tex_id.0,
                        sampler_index: sampler_id,
                        flags: 0,
                        opacity: 1.0,
                    });
                }
            }
        }

        // 4. Prepare Overlay Layer (if present)
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
                        tf,
                        ow,
                        oh,
                        overlay_pixels,
                        false,
                    )?;
                } else if tex.version != overlay.version {
                    tex.upload_pixels(&self.queue, tf, overlay_pixels, false)?;
                    tex.version = overlay.version;
                }
            } else {
                self.overlay_texture = Some(GpuTexture::new(
                    &self.device,
                    &self.queue,
                    tf,
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

                if let Some(tex_id) = tex.texture.id() {
                    quads.push(QuadPush {
                        rect: [ndc_x0, ndc_y0, ndc_x1, ndc_y1],
                        uv_rect: [0.0, 0.0, 1.0, 1.0],
                        tex_index: tex_id.0,
                        sampler_index: sampler_id,
                        flags: 0,
                        opacity: 1.0,
                    });
                }
            }
        }

        // 5. Execute Render Pass with Dynamic Rendering
        let mut encoder = self
            .device
            .create_command_encoder()
            .map_err(|e| format!("Failed to create lgpu command encoder: {e}"))?;

        if quads.is_empty() {
            encoder
                .clear_and_render_texture(&frame.texture, clear_color)
                .map_err(|e| format!("Failed to clear texture: {e}"))?;
        } else {
            let pass_desc = lgpu::RenderPassDesc {
                color_texture: &frame.texture,
                color_load_op: lgpu::ColorLoadOp::Clear(clear_color),
                depth_attachment: None,
            };
            let mut pass = encoder.begin_render_pass(pass_desc);
            for quad in &quads {
                pass.draw(&self.pipeline, 6, bytemuck::bytes_of(quad))
                    .map_err(|e| format!("Quad draw failed: {e}"))?;
            }
        }

        encoder.transition_to_present(&frame.texture);
        if let Err(e) = self.queue.submit_surface(encoder.finish(), frame) {
            let err_str = e.to_string();
            log::warn!("Swapchain submit_surface error, reconfiguring surface: {err_str}");
            let _ = self.surface.configure(&self.device, &self.config);
            return Ok(());
        }

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
            Ok(f) => f,
            Err(_) => {
                if let Err(e) = self.surface.configure(&self.device, &self.config) {
                    log::warn!("Surface reconfigure failed during frame acquire: {e}");
                    return Ok(());
                }
                match self.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(e) => {
                        log::warn!("lgpu get_current_texture failed: {e}");
                        return Ok(());
                    }
                }
            }
        };

        let mut pixels = vec![bg_color.to_rgb_u32(); (w * h) as usize];
        mango_render::painter::paint(dl, &mut pixels, w, h);

        let tf = self.texture_format;
        if let Some(tex) = &mut self.frame_texture {
            if tex.width != w || tex.height != h {
                *tex = GpuTexture::new(&self.device, &self.queue, tf, w, h, &pixels, true)?;
            } else {
                tex.upload_pixels(&self.queue, tf, &pixels, true)?;
            }
        } else {
            self.frame_texture = Some(GpuTexture::new(
                &self.device,
                &self.queue,
                tf,
                w,
                h,
                &pixels,
                true,
            )?);
        }

        let clear_color = [
            bg_color.r as f32 / 255.0,
            bg_color.g as f32 / 255.0,
            bg_color.b as f32 / 255.0,
            1.0,
        ];

        let sampler_id = self.sampler.id().map(|id| id.0).unwrap_or(0);
        let tex_id = self
            .frame_texture
            .as_ref()
            .and_then(|t| t.texture.id())
            .map(|id| id.0)
            .unwrap_or(0);

        let quad = QuadPush {
            rect: [-1.0, 1.0, 1.0, -1.0],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            tex_index: tex_id,
            sampler_index: sampler_id,
            flags: 1,
            opacity: 1.0,
        };

        let mut encoder = self
            .device
            .create_command_encoder()
            .map_err(|e| format!("Failed to create lgpu command encoder: {e}"))?;

        {
            let pass_desc = lgpu::RenderPassDesc {
                color_texture: &frame.texture,
                color_load_op: lgpu::ColorLoadOp::Clear(clear_color),
                depth_attachment: None,
            };
            let mut pass = encoder.begin_render_pass(pass_desc);
            pass.draw(&self.pipeline, 6, bytemuck::bytes_of(&quad))
                .map_err(|e| format!("Quad draw failed: {e}"))?;
        }

        encoder.transition_to_present(&frame.texture);
        if let Err(e) = self.queue.submit_surface(encoder.finish(), frame) {
            let err_str = e.to_string();
            log::warn!("Swapchain submit_surface error, reconfiguring surface: {err_str}");
            let _ = self.surface.configure(&self.device, &self.config);
            return Ok(());
        }

        Ok(())
    }
}
