//! Format decoders for PNG, JPEG, GIF, WebP, SVG, and data: URIs.

use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, GenericImageView};
use std::io::Cursor;

use super::image::{AnimatedImage, AnimationFrame, DecodedImage, ProgressiveScan};

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
        || (!is_base64
            && (data.starts_with("<svg")
                || data.starts_with("%3Csvg")
                || data.starts_with("%3C%21DOCTYPE")
                || data.starts_with("%3C%3Fxml")))
    {
        let svg_str = if is_base64 {
            let bytes = base64_decode(data)?;
            String::from_utf8(bytes).ok()?
        } else {
            percent_decode(data)
        };
        let (w_opt, h_opt) = crate::svg::get_svg_intrinsic_dimensions(&svg_str);
        // An SVG image without width/height/viewBox has no intrinsic
        // dimensions.  HTML replaced-element sizing supplies the default
        // object size of 300x150; using an icon-sized 24x24 fallback makes
        // flex/grid layout collapse and violates the image sizing rules.
        let w = w_opt.unwrap_or(300.0).max(1.0) as u32;
        let h = h_opt.unwrap_or(150.0).max(1.0) as u32;
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
    // Check limits before image::load_from_memory allocates a decompressed
    // pixel buffer. Compressed images with huge dimensions can otherwise
    // create multi-gigabyte peaks before the bounded cache gets a chance to
    // evict them.
    const MAX_COMPRESSED_IMAGE_BYTES: usize = 16 * 1024 * 1024;
    const MAX_DECODED_PIXELS: u64 = 16 * 1024 * 1024;
    if bytes.len() > MAX_COMPRESSED_IMAGE_BYTES {
        return None;
    }

    // Check if raw bytes are data URI or SVG XML
    if let Ok(text) = std::str::from_utf8(bytes) {
        let trimmed = text.trim();
        if trimmed.starts_with("data:") {
            return decode_data_uri(trimmed);
        }
        if trimmed.starts_with("<svg") || trimmed.starts_with("<?xml") {
            let (w_opt, h_opt) = crate::svg::get_svg_intrinsic_dimensions(trimmed);
            let w = w_opt.unwrap_or(300.0).max(1.0) as u32;
            let h = h_opt.unwrap_or(150.0).max(1.0) as u32;
            if (w as u64).saturating_mul(h as u64) > MAX_DECODED_PIXELS {
                return None;
            }
            if let Some(svg_img) = crate::svg::render_svg(trimmed, w, h, mango_core::Color::BLACK) {
                return Some(svg_img);
            }
        }
    }

    let img = image::load_from_memory(bytes).ok()?;
    let (width, height) = img.dimensions();
    if (width as u64).saturating_mul(height as u64) > MAX_DECODED_PIXELS {
        return None;
    }
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
        let img =
            decode_image_bytes(bytes).ok_or_else(|| "Failed to decode JPEG bytes".to_string())?;
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
    if (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"))
        && let Some(anim) = decode_animated_gif(bytes)
    {
        return Some(anim);
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
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
        {
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
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let (chunks, rem) = input.as_chunks::<3>();
    for chunk in chunks {
        let b0 = chunk[0] as usize;
        let b1 = chunk[1] as usize;
        let b2 = chunk[2] as usize;
        out.push(CHARSET[b0 >> 2] as char);
        out.push(CHARSET[((b0 & 3) << 4) | (b1 >> 4)] as char);
        out.push(CHARSET[((b1 & 15) << 2) | (b2 >> 6)] as char);
        out.push(CHARSET[b2 & 63] as char);
    }
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
