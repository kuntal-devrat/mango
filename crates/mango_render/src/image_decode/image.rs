//! Decoded image buffers, progressive scans, and animation structures.

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

impl DecodedImage {
    /// Resizes the image to `(target_w, target_h)` using high-quality anti-aliased resampling.
    ///
    /// Uses an area-averaging box filter with premultiplied alpha for downscaling
    /// to eliminate pixelation and jagged aliasing, and bilinear interpolation for upscaling.
    pub fn resize(&self, target_w: u32, target_h: u32) -> DecodedImage {
        if self.width == target_w && self.height == target_h {
            return self.clone();
        }
        if target_w == 0
            || target_h == 0
            || self.width == 0
            || self.height == 0
            || self.pixels.is_empty()
        {
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

            contribs.push(ResampleContrib {
                start,
                end,
                weights,
            });
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
