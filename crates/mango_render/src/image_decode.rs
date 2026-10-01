//! Image decoding: data URI parsing and PNG/JPEG/GIF/WebP image decoding.
//!
//! Supports `data:image/*;base64,...` URIs for inline images. HTTP image
//! fetching will be added in Phase 3 (Networking).

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::RwLock;

use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, GenericImageView};

static DECODED_IMAGE_CACHE: std::sync::OnceLock<RwLock<HashMap<String, DecodedImage>>> =
    std::sync::OnceLock::new();
static DECODE_IN_FLIGHT: std::sync::OnceLock<RwLock<HashSet<String>>> =
    std::sync::OnceLock::new();

static SCALED_IMAGE_CACHE: std::sync::OnceLock<RwLock<HashMap<(String, u32, u32), DecodedImage>>> =
    std::sync::OnceLock::new();

fn global_image_cache() -> &'static RwLock<HashMap<String, DecodedImage>> {
    DECODED_IMAGE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn global_scaled_cache() -> &'static RwLock<HashMap<(String, u32, u32), DecodedImage>> {
    SCALED_IMAGE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn in_flight_set() -> &'static RwLock<HashSet<String>> {
    DECODE_IN_FLIGHT.get_or_init(|| RwLock::new(HashSet::new()))
}

/// Stores a decoded image in the global image cache.
pub fn cache_image(key: &str, img: DecodedImage) {
    if let Ok(mut cache) = global_image_cache().write() {
        cache.insert(key.to_string(), img);
    }
}

/// Retrieves a scaled image from the scaled image cache by (url, target_width, target_height).
pub fn get_scaled_image(key: &str, target_w: u32, target_h: u32) -> Option<DecodedImage> {
    let cache = global_scaled_cache().read().ok()?;
    cache.get(&(key.to_string(), target_w, target_h)).cloned()
}

/// Stores a scaled image in the scaled image cache.
pub fn cache_scaled_image(key: &str, target_w: u32, target_h: u32, img: DecodedImage) {
    if let Ok(mut cache) = global_scaled_cache().write() {
        if cache.len() > 512 {
            cache.clear();
        }
        cache.insert((key.to_string(), target_w, target_h), img);
    }
}

/// Gets an existing scaled image or scales the provided image and caches the result.
pub fn get_or_resize_cached(key: &str, img: &DecodedImage, target_w: u32, target_h: u32) -> DecodedImage {
    if let Some(scaled) = get_scaled_image(key, target_w, target_h) {
        return scaled;
    }
    let scaled = img.resize(target_w, target_h);
    cache_scaled_image(key, target_w, target_h, scaled.clone());
    scaled
}

/// Triggers asynchronous decoding of image bytes in a background worker thread (OPT-006).
/// Once decoded, the image is automatically inserted into the global image cache.
pub fn decode_image_async(key: String, bytes: Vec<u8>) {
    if get_cached_image(&key).is_some() {
        return;
    }
    if let Ok(mut in_flight) = in_flight_set().write() {
        if !in_flight.insert(key.clone()) {
            return; // Decode already in progress
        }
    }

    std::thread::Builder::new()
        .name("mango-img-decode".to_string())
        .spawn(move || {
            if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
                if let Some(anim) = decode_animated_gif(&bytes) {
                    if anim.frames.len() > 1 {
                        cache_animated_image(&key, anim.clone());
                    }
                    if let Some(first) = anim.frames.first() {
                        cache_image(&key, first.image.clone());
                    }
                } else if let Some(decoded) = decode_image_bytes(&bytes) {
                    cache_image(&key, decoded);
                }
            } else if let Some(decoded) = decode_image_bytes(&bytes) {
                cache_image(&key, decoded);
            }
            if let Ok(mut in_flight) = in_flight_set().write() {
                in_flight.remove(&key);
            }
        })
        .ok();
}

/// Retrieves a decoded image from cache, or initiates background decoding
/// while returning a placeholder image immediately to avoid blocking the main event loop (OPT-006).
pub fn get_or_decode_async(
    key: &str,
    bytes: &[u8],
    placeholder_w: u32,
    placeholder_h: u32,
) -> DecodedImage {
    if let Some(img) = get_cached_image(key) {
        return img;
    }
    decode_image_async(key.to_string(), bytes.to_vec());
    create_placeholder(placeholder_w, placeholder_h)
}

/// Retrieves a decoded image from the global image cache by key/URL.
/// Supports exact match, protocol-relative (`//`), query/fragment stripping, and path suffixes.
pub fn get_cached_image(key: &str) -> Option<DecodedImage> {
    let cache = global_image_cache().read().ok()?;
    if let Some(img) = cache.get(key) {
        return Some(img.clone());
    }
    if key.starts_with("//") {
        let https_key = format!("https:{key}");
        if let Some(img) = cache.get(&https_key) {
            return Some(img.clone());
        }
    }
    let clean_key = key.split('?').next().unwrap_or(key).split('#').next().unwrap_or(key);
    if let Some(img) = cache.get(clean_key) {
        return Some(img.clone());
    }
    if key.starts_with('/') {
        for (k, img) in cache.iter() {
            if k.ends_with(key) {
                return Some(img.clone());
            }
        }
    }
    None
}


/// A decoded image ready for framebuffer blitting.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel data in `0xRRGGBB` format, row-major, length = width * height.
    pub pixels: Vec<u32>,
}

/// A decoded progressive scan pass of a JPEG image.
#[derive(Debug, Clone)]
pub struct ProgressiveScan {
    /// 0-indexed scan pass number.
    pub scan_index: usize,
    /// Whether this is the final, full-fidelity pass.
    pub is_final: bool,
    /// Image representation for this scan.
    pub image: DecodedImage,
}

/// A single frame of an animated image with its display duration.
#[derive(Debug, Clone)]
pub struct AnimationFrame {
    /// Decoded framebuffer image for this frame.
    pub image: DecodedImage,
    /// Duration to display this frame in milliseconds.
    pub delay_ms: u32,
}

/// An animated image containing multiple sequential frames and looping metrics.
#[derive(Debug, Clone)]
pub struct AnimatedImage {
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Ordered frames comprising the animation.
    pub frames: Vec<AnimationFrame>,
    /// Loop count: 0 indicates infinite looping (standard for web GIF/APNG/WebP).
    pub loop_count: u32,
    /// Total duration of one animation cycle in milliseconds.
    pub total_duration_ms: u32,
}

impl AnimatedImage {
    /// Returns the frame corresponding to elapsed time `elapsed_ms`.
    pub fn frame_at_time(&self, elapsed_ms: u64) -> &DecodedImage {
        if self.frames.is_empty() {
            panic!("AnimatedImage has no frames");
        }
        if self.frames.len() == 1 || self.total_duration_ms == 0 {
            return &self.frames[0].image;
        }

        let cycle_time = (elapsed_ms % (self.total_duration_ms as u64)) as u32;
        let mut accum = 0u32;
        for frame in &self.frames {
            accum += frame.delay_ms;
            if cycle_time < accum {
                return &frame.image;
            }
        }
        &self.frames.last().unwrap().image
    }
}

static ANIMATED_IMAGE_CACHE: std::sync::OnceLock<RwLock<HashMap<String, AnimatedImage>>> =
    std::sync::OnceLock::new();

fn global_animated_cache() -> &'static RwLock<HashMap<String, AnimatedImage>> {
    ANIMATED_IMAGE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Stores an animated image in the cache.
pub fn cache_animated_image(key: &str, anim: AnimatedImage) {
    if let Ok(mut cache) = global_animated_cache().write() {
        cache.insert(key.to_string(), anim);
    }
}

/// Retrieves an animated image from the cache.
pub fn get_cached_animated_image(key: &str) -> Option<AnimatedImage> {
    let cache = global_animated_cache().read().ok()?;
    cache.get(key).cloned()
}

/// Retrieves the active frame of an animated image for elapsed time `elapsed_ms`.
pub fn get_animated_frame(key: &str, elapsed_ms: u64) -> Option<DecodedImage> {
    let anim = get_cached_animated_image(key)?;
    Some(anim.frame_at_time(elapsed_ms).clone())
}

impl DecodedImage {
    /// Resizes the image to `(target_w, target_h)` using high-quality anti-aliased resampling.
    ///
    /// Uses an area-averaging box filter with premultiplied alpha for downscaling
    /// to eliminate pixelation and jagged aliasing, and bilinear interpolation for upscaling.
    pub fn resize(&self, target_w: u32, target_h: u32) -> DecodedImage {
        if self.width == target_w && self.height == target_h {
            return self.clone();
        }
        if target_w == 0 || target_h == 0 || self.width == 0 || self.height == 0 || self.pixels.is_empty() {
            return DecodedImage {
                width: target_w,
                height: target_h,
                pixels: vec![0; (target_w * target_h) as usize],
            };
        }

        let src_w = self.width as usize;
        let src_h = self.height as usize;
        let dst_w = target_w as usize;
        let dst_h = target_h as usize;

        let has_alpha = self.pixels.iter().any(|&p| {
            let a = (p >> 24) & 0xFF;
            a > 0 && a < 255
        });
        let all_zero_alpha = self.pixels.iter().all(|&p| (p >> 24) == 0);

        // Precompute horizontal weights
        let h_contribs = compute_resample_weights(src_w, dst_w);
        // Precompute vertical weights
        let v_contribs = compute_resample_weights(src_h, dst_h);

        // Intermediate buffer: (dst_w x src_h) with 4 float channels [pr, pg, pb, a]
        let mut intermediate = vec![[0.0f32; 4]; dst_w * src_h];

        for sy in 0..src_h {
            let row_offset = sy * src_w;
            let out_row_offset = sy * dst_w;

            for (dx, contrib) in h_contribs.iter().enumerate() {
                let mut sum_pr = 0.0f32;
                let mut sum_pg = 0.0f32;
                let mut sum_pb = 0.0f32;
                let mut sum_a = 0.0f32;

                for (idx, &w) in (contrib.start..contrib.end).zip(contrib.weights.iter()) {
                    let pixel = self.pixels.get(row_offset + idx).copied().unwrap_or(0);
                    let a = if all_zero_alpha {
                        255.0
                    } else {
                        ((pixel >> 24) & 0xFF) as f32
                    };
                    let r = ((pixel >> 16) & 0xFF) as f32;
                    let g = ((pixel >> 8) & 0xFF) as f32;
                    let b = (pixel & 0xFF) as f32;
                    let alpha_norm = a / 255.0;

                    sum_pr += r * alpha_norm * w;
                    sum_pg += g * alpha_norm * w;
                    sum_pb += b * alpha_norm * w;
                    sum_a += a * w;
                }

                intermediate[out_row_offset + dx] = [sum_pr, sum_pg, sum_pb, sum_a];
            }
        }

        // Final output buffer: (dst_w x dst_h) packed into canonical 0xAARRGGBB
        let mut pixels = vec![0u32; dst_w * dst_h];

        for (dy, contrib) in v_contribs.iter().enumerate() {
            let out_row_offset = dy * dst_w;

            for dx in 0..dst_w {
                let mut sum_pr = 0.0f32;
                let mut sum_pg = 0.0f32;
                let mut sum_pb = 0.0f32;
                let mut sum_a = 0.0f32;

                for (sy, &w) in (contrib.start..contrib.end).zip(contrib.weights.iter()) {
                    let sample = intermediate[sy * dst_w + dx];
                    sum_pr += sample[0] * w;
                    sum_pg += sample[1] * w;
                    sum_pb += sample[2] * w;
                    sum_a += sample[3] * w;
                }

                let final_a = if all_zero_alpha && !has_alpha {
                    255
                } else {
                    sum_a.clamp(0.0, 255.0).round() as u32
                };
                let (final_r, final_g, final_b) = if final_a > 0 {
                    let alpha_norm = sum_a.max(1.0) / 255.0;
                    (
                        (sum_pr / alpha_norm).clamp(0.0, 255.0).round() as u32,
                        (sum_pg / alpha_norm).clamp(0.0, 255.0).round() as u32,
                        (sum_pb / alpha_norm).clamp(0.0, 255.0).round() as u32,
                    )
                } else {
                    (0, 0, 0)
                };

                let packed = (final_a << 24) | (final_r << 16) | (final_g << 8) | final_b;
                pixels[out_row_offset + dx] = packed;
            }
        }

        DecodedImage {
            width: target_w,
            height: target_h,
            pixels,
        }
    }
}

struct ResampleContrib {
    start: usize,
    end: usize,
    weights: Vec<f32>,
}

fn compute_resample_weights(src_len: usize, dst_len: usize) -> Vec<ResampleContrib> {
    let mut contribs = Vec::with_capacity(dst_len);

    if dst_len <= src_len {
        // Downscaling: Box filter area-averaging
        let scale = src_len as f32 / dst_len as f32;
        for d in 0..dst_len {
            let start_f = d as f32 * scale;
            let end_f = (d + 1) as f32 * scale;
            let start = (start_f.floor() as usize).min(src_len.saturating_sub(1));
            let end = (end_f.ceil() as usize).min(src_len).max(start + 1);

            let mut weights = Vec::with_capacity(end - start);
            let mut total_w = 0.0f32;
            for s in start..end {
                let left = (s as f32).max(start_f);
                let right = (s as f32 + 1.0).min(end_f);
                let w = (right - left).max(0.0);
                weights.push(w);
                total_w += w;
            }

            if total_w > 0.0 {
                for w in &mut weights {
                    *w /= total_w;
                }
            } else if !weights.is_empty() {
                weights[0] = 1.0;
            }

            contribs.push(ResampleContrib { start, end, weights });
        }
    } else {
        // Upscaling: Bilinear interpolation
        let scale = src_len as f32 / dst_len as f32;
        for d in 0..dst_len {
            let center = (d as f32 + 0.5) * scale - 0.5;
            let clamped = center.clamp(0.0, (src_len.saturating_sub(1)) as f32);
            let s0 = clamped.floor() as usize;
            let s1 = (s0 + 1).min(src_len.saturating_sub(1));
            let frac = clamped - s0 as f32;

            if s0 == s1 {
                contribs.push(ResampleContrib {
                    start: s0,
                    end: s0 + 1,
                    weights: vec![1.0],
                });
            } else {
                contribs.push(ResampleContrib {
                    start: s0,
                    end: s1 + 1,
                    weights: vec![1.0 - frac, frac],
                });
            }
        }
    }

    contribs
}

/// Decodes a `data:` URI containing an image (base64 or SVG).
///
/// Supports `data:image/svg+xml,...`, `data:image/png;base64,...`,
/// `data:image/jpeg;base64,...`, `data:image/gif;base64,...`, and `data:image/webp;base64,...`.
///
/// Returns `None` if the URI is not a valid data URI or the image cannot be decoded.
pub fn decode_data_uri(uri: &str) -> Option<DecodedImage> {
    let uri = uri.trim();

    // Parse data: URI format
    let rest = uri.strip_prefix("data:")?;
    let (metadata, data) = rest.split_once(',')?;

    let is_base64 = metadata
        .split(';')
        .any(|part| part.trim().eq_ignore_ascii_case("base64"));
    let mime = metadata
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();

    if mime == "image/svg+xml"
        || (!is_base64 && (data.starts_with("<svg") || data.starts_with("%3Csvg") || data.starts_with("%3C%21DOCTYPE") || data.starts_with("%3C%3Fxml")))
    {
        let svg_str = if is_base64 {
            let bytes = base64_decode(data)?;
            String::from_utf8(bytes).ok()?
        } else {
            percent_decode(data)
        };
        let (w_opt, h_opt) = crate::svg::get_svg_intrinsic_dimensions(&svg_str);
        let w = w_opt.unwrap_or(24.0).max(1.0) as u32;
        let h = h_opt.unwrap_or(24.0).max(1.0) as u32;
        return crate::svg::render_svg(&svg_str, w, h, mango_core::Color::BLACK);
    }

    if is_base64 {
        let bytes = base64_decode(data)?;
        decode_image_bytes(&bytes)
    } else {
        None
    }
}

/// Decodes raw image bytes (PNG, JPEG, GIF, WebP, SVG, or data URI) into a `DecodedImage`.
pub fn decode_image_bytes(bytes: &[u8]) -> Option<DecodedImage> {
    // Check if raw bytes are data URI or SVG XML
    if let Ok(text) = std::str::from_utf8(bytes) {
        let trimmed = text.trim();
        if trimmed.starts_with("data:") {
            return decode_data_uri(trimmed);
        }
        if trimmed.starts_with("<svg") || trimmed.starts_with("<?xml") {
            let (w_opt, h_opt) = crate::svg::get_svg_intrinsic_dimensions(trimmed);
            let w = w_opt.unwrap_or(32.0).max(1.0) as u32;
            let h = h_opt.unwrap_or(32.0).max(1.0) as u32;
            if let Some(svg_img) = crate::svg::render_svg(trimmed, w, h, mango_core::Color::BLACK) {
                return Some(svg_img);
            }
        }
    }

    let img = image::load_from_memory(bytes).ok()?;
    let (width, height) = img.dimensions();
    let rgba = img.to_rgba8();

    let mut pixels = Vec::with_capacity((width * height) as usize);
    for pixel in rgba.pixels() {
        let [r, g, b, a] = pixel.0;
        pixels.push(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
    }

    Some(DecodedImage {
        width,
        height,
        pixels,
    })
}

/// Detects whether raw image bytes represent a progressive JPEG (SOF2 marker 0xFF 0xC2).
pub fn is_progressive_jpeg(bytes: &[u8]) -> bool {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return false;
    }
    let mut i = 2;
    while i + 1 < bytes.len() {
        if bytes[i] == 0xFF {
            let marker = bytes[i + 1];
            if marker == 0xC2 {
                return true; // SOF2: Progressive DCT
            }
            if marker == 0xC0 {
                return false; // SOF0: Baseline DCT
            }
            if marker == 0xDA || marker == 0xD9 {
                // SOS or EOI reached without SOF2
                return false;
            }
            if marker != 0x00 && marker != 0xFF {
                // Read marker segment length
                if i + 3 < bytes.len() {
                    let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
                    i += 2 + len;
                    continue;
                }
            }
        }
        i += 1;
    }
    false
}

/// Decodes progressive JPEG scans from bytes.
/// Returns a sequence of progressive scan approximations (from coarse preview to final crisp image).
pub fn decode_progressive_jpeg_scans(bytes: &[u8]) -> Result<Vec<ProgressiveScan>, String> {
    if !is_progressive_jpeg(bytes) {
        let img = decode_image_bytes(bytes)
            .ok_or_else(|| "Failed to decode JPEG bytes".to_string())?;
        return Ok(vec![ProgressiveScan {
            scan_index: 0,
            is_final: true,
            image: img,
        }]);
    }

    let full_img = decode_image_bytes(bytes)
        .ok_or_else(|| "Failed to decode progressive JPEG full pass".to_string())?;

    let w = full_img.width;
    let h = full_img.height;
    if w == 0 || h == 0 {
        return Ok(vec![ProgressiveScan {
            scan_index: 0,
            is_final: true,
            image: full_img,
        }]);
    }

    let mut scans = Vec::new();

    // Pass 0: Low-frequency DC scan approximation (rough preview block-averaged and smoothed)
    let dc_w = (w / 8).max(4);
    let dc_h = (h / 8).max(4);
    let dc_thumb = full_img.resize(dc_w, dc_h);
    let pass0 = dc_thumb.resize(w, h);
    scans.push(ProgressiveScan {
        scan_index: 0,
        is_final: false,
        image: pass0,
    });

    // Pass 1: Intermediate spectral/AC scan approximation (medium detail pass)
    let mid_w = (w / 2).max(8);
    let mid_h = (h / 2).max(8);
    let mid_thumb = full_img.resize(mid_w, mid_h);
    let pass1 = mid_thumb.resize(w, h);
    scans.push(ProgressiveScan {
        scan_index: 1,
        is_final: false,
        image: pass1,
    });

    // Pass 2: Final full-fidelity scan pass
    scans.push(ProgressiveScan {
        scan_index: 2,
        is_final: true,
        image: full_img,
    });

    Ok(scans)
}

/// Decodes an animated GIF into an `AnimatedImage` with all sequential frames and durations.
pub fn decode_animated_gif(bytes: &[u8]) -> Option<AnimatedImage> {
    if !bytes.starts_with(b"GIF87a") && !bytes.starts_with(b"GIF89a") {
        return None;
    }
    let decoder = GifDecoder::new(Cursor::new(bytes)).ok()?;
    let frames = decoder.into_frames().collect_frames().ok()?;
    if frames.is_empty() {
        return None;
    }

    let mut anim_frames = Vec::with_capacity(frames.len());
    let mut total_duration_ms = 0u32;

    for frame in frames {
        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay_ms = if denom > 0 {
            ((numer as f32 / denom as f32).round() as u32).max(10)
        } else {
            100
        };
        total_duration_ms += delay_ms;

        let buffer = frame.into_buffer();
        let (width, height) = buffer.dimensions();
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for pixel in buffer.pixels() {
            let [r, g, b, a] = pixel.0;
            pixels.push(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
        }

        anim_frames.push(AnimationFrame {
            image: DecodedImage {
                width,
                height,
                pixels,
            },
            delay_ms,
        });
    }

    let width = anim_frames[0].image.width;
    let height = anim_frames[0].image.height;

    Some(AnimatedImage {
        width,
        height,
        frames: anim_frames,
        loop_count: 0,
        total_duration_ms,
    })
}

/// Decodes an animated image from raw bytes (GIF, APNG, WebP, or fallback single frame).
pub fn decode_animated_image_bytes(bytes: &[u8]) -> Option<AnimatedImage> {
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        if let Some(anim) = decode_animated_gif(bytes) {
            return Some(anim);
        }
    }

    let single = decode_image_bytes(bytes)?;
    Some(AnimatedImage {
        width: single.width,
        height: single.height,
        frames: vec![AnimationFrame {
            image: single,
            delay_ms: 100,
        }],
        loop_count: 0,
        total_duration_ms: 100,
    })
}

fn percent_decode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(byte as char);
                i += 3;
                continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Generates a placeholder image for missing or failed image loads.
///
/// Creates a light grey rectangle with a darker border and a small "×" icon.
pub fn create_placeholder(width: u32, height: u32) -> DecodedImage {
    let w = width.max(16);
    let h = height.max(16);
    let bg_color = 0xFFF0F0F0u32;
    let mut pixels = vec![bg_color; (w * h) as usize];

    // Draw border (1px dark grey)
    let border_color = 0xFFCCCCCCu32;
    for x in 0..w {
        pixels[x as usize] = border_color; // top
        pixels[((h - 1) * w + x) as usize] = border_color; // bottom
    }
    for y in 0..h {
        pixels[(y * w) as usize] = border_color; // left
        pixels[(y * w + w - 1) as usize] = border_color; // right
    }

    // Draw a small "×" icon in the center (broken image indicator)
    let cx = w / 2;
    let cy = h / 2;
    let icon_size = (w.min(h) / 4).clamp(3, 12);
    let icon_color = 0xFF999999u32;

    for i in 0..icon_size {
        let xi = i as i32;
        let half = icon_size as i32 / 2;

        // Diagonal \
        let px1 = (cx as i32 - half + xi) as u32;
        let py1 = (cy as i32 - half + xi) as u32;
        if px1 < w && py1 < h {
            pixels[(py1 * w + px1) as usize] = icon_color;
        }

        // Diagonal /
        let px2 = (cx as i32 + half - xi) as u32;
        let py2 = (cy as i32 - half + xi) as u32;
        if px2 < w && py2 < h {
            pixels[(py2 * w + px2) as usize] = icon_color;
        }
    }

    DecodedImage {
        width: w,
        height: h,
        pixels,
    }
}

/// Simple base64 decoder (no padding required).
pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    // Strip whitespace
    let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();

    let mut output = Vec::with_capacity(clean.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;

    for ch in clean.chars() {
        let val = match ch {
            'A'..='Z' => ch as u32 - b'A' as u32,
            'a'..='z' => ch as u32 - b'a' as u32 + 26,
            '0'..='9' => ch as u32 - b'0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            '=' => continue, // padding — skip
            _ => return None,
        };

        buf = (buf << 6) | val;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            output.push(((buf >> bits) & 0xFF) as u8);
        }
    }

    Some(output)
}

/// Simple base64 encoder.
pub fn base64_encode(input: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    let mut chunks = input.chunks_exact(3);
    for chunk in chunks.by_ref() {
        let b0 = chunk[0] as usize;
        let b1 = chunk[1] as usize;
        let b2 = chunk[2] as usize;
        out.push(CHARSET[b0 >> 2] as char);
        out.push(CHARSET[((b0 & 3) << 4) | (b1 >> 4)] as char);
        out.push(CHARSET[((b1 & 15) << 2) | (b2 >> 6)] as char);
        out.push(CHARSET[b2 & 63] as char);
    }
    let rem = chunks.remainder();
    if rem.len() == 1 {
        let b0 = rem[0] as usize;
        out.push(CHARSET[b0 >> 2] as char);
        out.push(CHARSET[(b0 & 3) << 4] as char);
        out.push('=');
        out.push('=');
    } else if rem.len() == 2 {
        let b0 = rem[0] as usize;
        let b1 = rem[1] as usize;
        out.push(CHARSET[b0 >> 2] as char);
        out.push(CHARSET[((b0 & 3) << 4) | (b1 >> 4)] as char);
        out.push(CHARSET[(b1 & 15) << 2] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_decode() {
        // "Hello" in base64 is "SGVsbG8="
        let decoded = base64_decode("SGVsbG8=").unwrap();
        assert_eq!(&decoded, b"Hello");
    }

    #[test]
    fn test_base64_decode_no_padding() {
        let decoded = base64_decode("SGVsbG8").unwrap();
        assert_eq!(&decoded, b"Hello");
    }

    #[test]
    fn test_create_placeholder() {
        let img = create_placeholder(64, 48);
        assert_eq!(img.width, 64);
        assert_eq!(img.height, 48);
        assert_eq!(img.pixels.len(), (64 * 48) as usize);

        // Check border pixels
        assert_eq!(img.pixels[0], 0xFFCCCCCC); // top-left
        assert_eq!(img.pixels[63], 0xFFCCCCCC); // top-right
    }

    #[test]
    fn test_decode_data_uri_invalid() {
        assert!(decode_data_uri("not-a-data-uri").is_none());
        assert!(decode_data_uri("data:text/plain;base64,SGVsbG8=").is_none()); // text, not image
    }

    #[test]
    fn test_create_placeholder_minimum_size() {
        let img = create_placeholder(2, 2);
        // Should be clamped to 16x16 minimum
        assert!(img.width >= 16);
        assert!(img.height >= 16);
    }

    #[test]
    fn test_decode_tiny_png_data_uri() {
        // A valid 1x1 red PNG as base64 data URI
        let png_data = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        let result = decode_data_uri(png_data);
        assert!(result.is_some(), "Should decode a valid PNG data URI");
        let img = result.unwrap();
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        assert_eq!(img.pixels.len(), 1);
    }

    #[test]
    fn test_resize_image() {
        let original = DecodedImage {
            width: 2,
            height: 2,
            pixels: vec![0xFFFF0000, 0xFF00FF00, 0xFF0000FF, 0xFFFFFF00],
        };
        let scaled = original.resize(4, 4);
        assert_eq!(scaled.width, 4);
        assert_eq!(scaled.height, 4);
        assert_eq!(scaled.pixels.len(), 16);
        assert_eq!(scaled.pixels[0], 0xFFFF0000);
    }

    #[test]
    fn test_decode_svg_data_uri_with_charset() {
        let svg_data = "data:image/svg+xml;charset=utf-8,%3Csvg xmlns='http://www.w3.org/2000/svg' width='20' height='20'%3E%3Cpath d='M0 0h20v20H0z' fill='%23ff0000'/%3E%3C/svg%3E";
        let result = decode_data_uri(svg_data);
        assert!(result.is_some(), "Should decode SVG data URI with charset=utf-8");
        let img = result.unwrap();
        assert_eq!(img.width, 20);
        assert_eq!(img.height, 20);
    }

    #[test]
    fn test_decode_svg_data_uri_base64_with_charset() {
        let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><rect width='16' height='16' fill='blue'/></svg>";
        let b64 = base64_encode(svg.as_bytes());
        let uri = format!("data:image/svg+xml;charset=utf-8;base64,{b64}");
        let result = decode_data_uri(&uri);
        assert!(result.is_some(), "Should decode SVG data URI with charset and base64");
    }

    #[test]
    fn test_async_image_decode_and_placeholder() {
        let png_bytes = base64_decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==").unwrap();
        let key = "https://example.com/async_test_image.png";

        // Initial call returns placeholder immediately without blocking
        let initial = get_or_decode_async(key, &png_bytes, 32, 32);
        assert_eq!(initial.width, 32);
        assert_eq!(initial.height, 32);

        // Wait a short moment for the background worker to finish decoding
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Now image cache should contain the fully decoded 1x1 image
        let cached = get_cached_image(key).expect("Image should be in cache after async decode");
        assert_eq!(cached.width, 1);
        assert_eq!(cached.height, 1);
    }

    #[test]
    fn test_progressive_jpeg_detection_and_scans() {
        // Construct a synthetic JPEG with SOF2 (Progressive DCT) marker: 0xFF 0xC2
        let progressive_header = [
            0xFF, 0xD8, // SOI
            0xFF, 0xE0, 0x00, 0x10, // APP0 length 16
            0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
            0xFF, 0xC2, 0x00, 0x0B, // SOF2 (Progressive DCT), length 11
            0x08, 0x00, 0x10, 0x00, 0x10, 0x03, 0x01, 0x11, 0x00,
            0xFF, 0xDA, // SOS
            0x00, 0x00,
            0xFF, 0xD9, // EOI
        ];
        assert!(is_progressive_jpeg(&progressive_header));

        // Baseline JPEG with SOF0 (0xFF 0xC0)
        let baseline_header = [
            0xFF, 0xD8, // SOI
            0xFF, 0xC0, 0x00, 0x0B, // SOF0 (Baseline DCT)
            0x08, 0x00, 0x10, 0x00, 0x10, 0x03, 0x01, 0x11, 0x00,
            0xFF, 0xD9, // EOI
        ];
        assert!(!is_progressive_jpeg(&baseline_header));

        // Non-JPEG data
        assert!(!is_progressive_jpeg(b"not a jpeg"));
    }

    #[test]
    fn test_animated_image_playback() {
        let frame1 = DecodedImage {
            width: 10,
            height: 10,
            pixels: vec![0xFFFF0000; 100], // Red
        };
        let frame2 = DecodedImage {
            width: 10,
            height: 10,
            pixels: vec![0xFF00FF00; 100], // Green
        };
        let frame3 = DecodedImage {
            width: 10,
            height: 10,
            pixels: vec![0xFF0000FF; 100], // Blue
        };

        let anim = AnimatedImage {
            width: 10,
            height: 10,
            frames: vec![
                AnimationFrame {
                    image: frame1,
                    delay_ms: 100,
                },
                AnimationFrame {
                    image: frame2,
                    delay_ms: 150,
                },
                AnimationFrame {
                    image: frame3,
                    delay_ms: 200,
                },
            ],
            loop_count: 0,
            total_duration_ms: 450,
        };

        // At 0ms -> Frame 1 (Red)
        assert_eq!(anim.frame_at_time(0).pixels[0], 0xFFFF0000);
        // At 50ms -> Frame 1 (Red)
        assert_eq!(anim.frame_at_time(50).pixels[0], 0xFFFF0000);
        // At 120ms -> Frame 2 (Green)
        assert_eq!(anim.frame_at_time(120).pixels[0], 0xFF00FF00);
        // At 240ms -> Frame 2 (Green)
        assert_eq!(anim.frame_at_time(240).pixels[0], 0xFF00FF00);
        // At 300ms -> Frame 3 (Blue)
        assert_eq!(anim.frame_at_time(300).pixels[0], 0xFF0000FF);
        // At 460ms -> Loops back to Frame 1 (Red, since 460 % 450 = 10ms)
        assert_eq!(anim.frame_at_time(460).pixels[0], 0xFFFF0000);

        // Test caching
        let key = "https://example.com/test.gif";
        cache_animated_image(key, anim);
        let cached_anim = get_cached_animated_image(key);
        assert!(cached_anim.is_some());
        let frame = get_animated_frame(key, 120);
        assert!(frame.is_some());
        assert_eq!(frame.unwrap().pixels[0], 0xFF00FF00);
    }
}
