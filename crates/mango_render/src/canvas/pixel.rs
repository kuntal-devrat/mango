//! Canvas 2D direct pixel manipulation, snapshot export, and framebuffer extraction.

use super::context::Canvas2D;
use crate::image_decode::base64_encode;

impl Canvas2D {
    // --- Pixel Manipulation ---

    /// Extracts unpremultiplied RGBA byte array of the specified rectangle.
    pub fn get_image_data(&self, sx: i32, sy: i32, sw: u32, sh: u32) -> Vec<u8> {
        let mut out = vec![0u8; (sw * sh * 4) as usize];
        let pixmap_data = self.pixmap.data();
        let w = self.width as i32;
        let h = self.height as i32;

        for y in 0..sh as i32 {
            let src_y = sy + y;
            if src_y < 0 || src_y >= h {
                continue;
            }
            for x in 0..sw as i32 {
                let src_x = sx + x;
                if src_x < 0 || src_x >= w {
                    continue;
                }
                let src_idx = ((src_y * w + src_x) * 4) as usize;
                let dst_idx = ((y * sw as i32 + x) * 4) as usize;
                let a = pixmap_data[src_idx + 3];
                if a == 0 {
                    out[dst_idx..dst_idx + 4].copy_from_slice(&[0, 0, 0, 0]);
                } else if a == 255 {
                    out[dst_idx..dst_idx + 4].copy_from_slice(&pixmap_data[src_idx..src_idx + 4]);
                } else {
                    let a_f = a as f32 / 255.0;
                    let r = ((pixmap_data[src_idx] as f32 / a_f).min(255.0).round()) as u8;
                    let g = ((pixmap_data[src_idx + 1] as f32 / a_f).min(255.0).round()) as u8;
                    let b = ((pixmap_data[src_idx + 2] as f32 / a_f).min(255.0).round()) as u8;
                    out[dst_idx] = r;
                    out[dst_idx + 1] = g;
                    out[dst_idx + 2] = b;
                    out[dst_idx + 3] = a;
                }
            }
        }
        out
    }

    /// Writes raw RGBA byte data into the canvas buffer at `(dx, dy)`.
    pub fn put_image_data(
        &mut self,
        data: &[u8],
        dx: i32,
        dy: i32,
        dirty_x: i32,
        dirty_y: i32,
        dirty_w: u32,
        dirty_h: u32,
    ) {
        self.put_image_data_with_stride(data, 0, dx, dy, dirty_x, dirty_y, dirty_w, dirty_h);
    }

    /// Writes raw RGBA byte data with explicit source image width into the canvas buffer at `(dx, dy)`.
    pub fn put_image_data_with_stride(
        &mut self,
        data: &[u8],
        src_width: u32,
        dx: i32,
        dy: i32,
        dirty_x: i32,
        dirty_y: i32,
        dirty_w: u32,
        dirty_h: u32,
    ) {
        let pm_data = self.pixmap.data_mut();
        let w = self.width as i32;
        let h = self.height as i32;

        let total_pixels = (data.len() / 4) as i32;
        let src_w = if src_width > 0 {
            src_width as i32
        } else if total_pixels == (dirty_w * dirty_h) as i32 {
            dirty_w as i32
        } else if dirty_w > 0 && total_pixels > (dirty_w * dirty_h) as i32 {
            (total_pixels / (dirty_y + dirty_h as i32).max(1)).max(dirty_w as i32)
        } else {
            dirty_w as i32
        };

        for y in dirty_y..(dirty_y + dirty_h as i32) {
            let target_y = dy + y;
            if target_y < 0 || target_y >= h {
                continue;
            }
            for x in dirty_x..(dirty_x + dirty_w as i32) {
                let target_x = dx + x;
                if target_x < 0 || target_x >= w {
                    continue;
                }
                let src_idx = ((y * src_w + x) * 4) as usize;
                if src_idx + 4 > data.len() {
                    continue;
                }
                let r = data[src_idx];
                let g = data[src_idx + 1];
                let b = data[src_idx + 2];
                let a = data[src_idx + 3];

                let dst_idx = ((target_y * w + target_x) * 4) as usize;
                if a == 0 {
                    pm_data[dst_idx..dst_idx + 4].copy_from_slice(&[0, 0, 0, 0]);
                } else if a == 255 {
                    pm_data[dst_idx] = r;
                    pm_data[dst_idx + 1] = g;
                    pm_data[dst_idx + 2] = b;
                    pm_data[dst_idx + 3] = 255;
                } else {
                    let a_f = a as f32 / 255.0;
                    pm_data[dst_idx] = ((r as f32 * a_f).round()) as u8;
                    pm_data[dst_idx + 1] = ((g as f32 * a_f).round()) as u8;
                    pm_data[dst_idx + 2] = ((b as f32 * a_f).round()) as u8;
                    pm_data[dst_idx + 3] = a;
                }
            }
        }
    }

    /// Encodes the current canvas bitmap as a PNG base64 `data:` URI.
    pub fn to_data_url(&self, _mime_type: &str) -> String {
        let raw_rgba = self.get_image_data(0, 0, self.width, self.height);
        if let Some(img) = image::RgbaImage::from_raw(self.width, self.height, raw_rgba) {
            let mut png_bytes = Vec::new();
            let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
            if img.write_with_encoder(encoder).is_ok() {
                let b64 = base64_encode(&png_bytes);
                return format!("data:image/png;base64,{b64}");
            }
        }
        "data:image/png;base64,".to_string()
    }

    /// Converts the current canvas pixmap to row-major `0xAARRGGBB` pixels for framebuffer blitting.
    pub fn get_pixels(&self) -> Vec<u32> {
        let mut pixels = Vec::with_capacity((self.width * self.height) as usize);
        for pixel in self.pixmap.pixels() {
            let a = pixel.alpha();
            if a == 0 {
                pixels.push(0);
            } else if a == 255 {
                pixels.push(
                    0xFF00_0000
                        | ((pixel.red() as u32) << 16)
                        | ((pixel.green() as u32) << 8)
                        | (pixel.blue() as u32),
                );
            } else {
                let a_f = a as f32 / 255.0;
                let r = ((pixel.red() as f32 / a_f).min(255.0).round()) as u32;
                let g = ((pixel.green() as f32 / a_f).min(255.0).round()) as u32;
                let b = ((pixel.blue() as f32 / a_f).min(255.0).round()) as u32;
                pixels.push(((a as u32) << 24) | (r << 16) | (g << 8) | b);
            }
        }
        pixels
    }
}
