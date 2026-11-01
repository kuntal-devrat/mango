//! SIMD-accelerated pixel operations, alpha blending, and filter effects.
//!
//! Provides vectorized 4-to-8 pixel batching using architecture intrinsics
//! (x86_64 SSE2/AVX2 and aarch64 NEON) with a portable fallback.

use mango_core::Color;

/// Blends a contiguous slice of destination pixels with a constant source color using SIMD.
pub fn blend_span_normal(dst: &mut [u32], color: Color) {
    let alpha = color.a as u32;
    if alpha == 0 || dst.is_empty() {
        return;
    }
    if alpha == 255 {
        dst.fill(color.to_rgb_u32());
        return;
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sse2") {
            unsafe {
                blend_span_sse2(dst, color);
                return;
            }
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        unsafe {
            blend_span_neon(dst, color);
            return;
        }
    }

    #[allow(unreachable_code)]
    blend_span_portable(dst, color);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_span_sse2(dst: &mut [u32], color: Color) {
    use std::arch::x86_64::*;

    let alpha = color.a as u16;
    let inv_a = 255 - alpha;
    let src_r = color.r as u16;
    let src_g = color.g as u16;
    let src_b = color.b as u16;

    // Bias of 127 for rounded division
    let bias_r = src_r * alpha + 127;
    let bias_g = src_g * alpha + 127;
    let bias_b = src_b * alpha + 127;

    unsafe {
        let inv_a_vec = _mm_set1_epi16(inv_a as i16);
        let zero = _mm_setzero_si128();

        // Constant addend: [0, b, g, r, 0, b, g, r] in 16-bit lanes
        let add_lo = _mm_setr_epi16(
            bias_b as i16,
            bias_g as i16,
            bias_r as i16,
            0,
            bias_b as i16,
            bias_g as i16,
            bias_r as i16,
            0,
        );
        let add_hi = add_lo;

        let len = dst.len();
        let mut i = 0;

        while i + 4 <= len {
            let ptr = dst.as_mut_ptr().add(i) as *mut __m128i;
            let d = _mm_loadu_si128(ptr);

            let d_lo = _mm_unpacklo_epi8(d, zero);
            let d_hi = _mm_unpackhi_epi8(d, zero);

            // d * inv_a
            let mul_lo = _mm_mullo_epi16(d_lo, inv_a_vec);
            let mul_hi = _mm_mullo_epi16(d_hi, inv_a_vec);

            // d * inv_a + src * alpha + 127
            let sum_lo = _mm_add_epi16(mul_lo, add_lo);
            let sum_hi = _mm_add_epi16(mul_hi, add_hi);

            // Fast divide by 255: (x + 1 + (x >> 8)) >> 8
            let div_lo = _mm_srli_epi16(
                _mm_add_epi16(
                    _mm_add_epi16(sum_lo, _mm_set1_epi16(1)),
                    _mm_srli_epi16(sum_lo, 8),
                ),
                8,
            );
            let div_hi = _mm_srli_epi16(
                _mm_add_epi16(
                    _mm_add_epi16(sum_hi, _mm_set1_epi16(1)),
                    _mm_srli_epi16(sum_hi, 8),
                ),
                8,
            );

            let res = _mm_packus_epi16(div_lo, div_hi);
            _mm_storeu_si128(ptr, res);

            i += 4;
        }

        // Scalar remainder
        while i < len {
            dst[i] = blend_scalar(color, dst[i]);
            i += 1;
        }
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn blend_span_neon(dst: &mut [u32], color: Color) {
    use std::arch::aarch64::*;

    let alpha = color.a as u16;
    let inv_a = (255 - alpha) as u8;
    let src_r = color.r as u16;
    let src_g = color.g as u16;
    let src_b = color.b as u16;

    let bias_r = src_r * alpha + 127;
    let bias_g = src_g * alpha + 127;
    let bias_b = src_b * alpha + 127;

    unsafe {
        let len = dst.len();
        let mut i = 0;

        let inv_a_vec = vdup_n_u8(inv_a);
        let add_vec = vcombine_u16(
            vcreate_u16(bias_b as u64 | ((bias_g as u64) << 16) | ((bias_r as u64) << 32)),
            vcreate_u16(bias_b as u64 | ((bias_g as u64) << 16) | ((bias_r as u64) << 32)),
        );

        while i + 4 <= len {
            let ptr = dst.as_mut_ptr().add(i) as *mut u8;
            let d = vld1_u8(ptr);
            let d_lo = vget_low_u8(d);
            let d_hi = vget_high_u8(d);

            let mul_lo = vmlal_u8(add_vec, d_lo, inv_a_vec);
            let mul_hi = vmlal_u8(add_vec, d_hi, inv_a_vec);

            let div_lo = vshrn_n_u16(mul_lo, 8);
            let div_hi = vshrn_n_u16(mul_hi, 8);

            let res = vcombine_u8(div_lo, div_hi);
            vst1_u8(ptr, res);

            i += 4;
        }

        while i < len {
            dst[i] = blend_scalar(color, dst[i]);
            i += 1;
        }
    }
}

fn blend_span_portable(dst: &mut [u32], color: Color) {
    let alpha = color.a as u32;
    let inv_a = 255 - alpha;
    let src_r = color.r as u32;
    let src_g = color.g as u32;
    let src_b = color.b as u32;

    let (chunks, remainder) = dst.as_chunks_mut::<4>();

    for chunk in chunks.iter_mut() {
        for p in chunk.iter_mut() {
            let dst_rgb = *p;
            let dst_r = (dst_rgb >> 16) & 0xFF;
            let dst_g = (dst_rgb >> 8) & 0xFF;
            let dst_b = dst_rgb & 0xFF;
            let out_r = (src_r * alpha + dst_r * inv_a + 127) / 255;
            let out_g = (src_g * alpha + dst_g * inv_a + 127) / 255;
            let out_b = (src_b * alpha + dst_b * inv_a + 127) / 255;
            *p = (out_r << 16) | (out_g << 8) | out_b;
        }
    }

    for p in remainder {
        *p = blend_scalar(color, *p);
    }
}

#[inline(always)]
fn blend_scalar(src: Color, dst_rgb: u32) -> u32 {
    let alpha = src.a as u32;
    let inv_a = 255 - alpha;
    let dst_r = (dst_rgb >> 16) & 0xFF;
    let dst_g = (dst_rgb >> 8) & 0xFF;
    let dst_b = dst_rgb & 0xFF;
    let out_r = (src.r as u32 * alpha + dst_r * inv_a + 127) / 255;
    let out_g = (src.g as u32 * alpha + dst_g * inv_a + 127) / 255;
    let out_b = (src.b as u32 * alpha + dst_b * inv_a + 127) / 255;
    (out_r << 16) | (out_g << 8) | out_b
}

/// Vectorized brightness filter across a slice of ARGB pixels.
pub fn apply_brightness_span(pixels: &mut [u32], factor: f32) {
    let factor_fixed = (factor * 256.0).clamp(0.0, 65535.0) as u32;
    for p in pixels.iter_mut() {
        let val = *p;
        let r = (((val >> 16) & 0xff) * factor_fixed) >> 8;
        let g = (((val >> 8) & 0xff) * factor_fixed) >> 8;
        let b = ((val & 0xff) * factor_fixed) >> 8;
        *p = (val & 0xff000000) | (r.min(255) << 16) | (g.min(255) << 8) | b.min(255);
    }
}

/// Vectorized contrast filter across a slice of ARGB pixels.
pub fn apply_contrast_span(pixels: &mut [u32], factor: f32) {
    for p in pixels.iter_mut() {
        let val = *p;
        let r = ((((val >> 16) & 0xff) as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u32;
        let g = ((((val >> 8) & 0xff) as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u32;
        let b = (((val & 0xff) as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u32;
        *p = (val & 0xff000000) | (r << 16) | (g << 8) | b;
    }
}

/// Vectorized grayscale filter across a slice of ARGB pixels.
pub fn apply_grayscale_span(pixels: &mut [u32], factor: f32) {
    let f = factor.clamp(0.0, 1.0);
    // 0.2126 * 256 ≈ 54, 0.7152 * 256 ≈ 183, 0.0722 * 256 ≈ 19
    let factor_q8 = (f * 256.0).round() as u32;
    for p in pixels.iter_mut() {
        let val = *p;
        let r = (val >> 16) & 0xff;
        let g = (val >> 8) & 0xff;
        let b = val & 0xff;
        let gray = (54 * r + 183 * g + 19 * b + 128) >> 8;
        let delta_r = ((gray as i32 - r as i32) * factor_q8 as i32) >> 8;
        let out_r = (r as i32 + delta_r).clamp(0, 255) as u32;
        let delta_g = ((gray as i32 - g as i32) * factor_q8 as i32) >> 8;
        let out_g = (g as i32 + delta_g).clamp(0, 255) as u32;
        let delta_b = ((gray as i32 - b as i32) * factor_q8 as i32) >> 8;
        let out_b = (b as i32 + delta_b).clamp(0, 255) as u32;
        *p = (val & 0xff000000) | (out_r << 16) | (out_g << 8) | out_b;
    }
}

/// Vectorized invert filter across a slice of ARGB pixels.
pub fn apply_invert_span(pixels: &mut [u32], factor: f32) {
    let f = factor.clamp(0.0, 1.0);
    let factor_q8 = (f * 256.0).round() as i32;
    for p in pixels.iter_mut() {
        let val = *p;
        let r = ((val >> 16) & 0xff) as i32;
        let g = ((val >> 8) & 0xff) as i32;
        let b = (val & 0xff) as i32;
        let out_r = (r + (((255 - 2 * r) * factor_q8) >> 8)).clamp(0, 255) as u32;
        let out_g = (g + (((255 - 2 * g) * factor_q8) >> 8)).clamp(0, 255) as u32;
        let out_b = (b + (((255 - 2 * b) * factor_q8) >> 8)).clamp(0, 255) as u32;
        *p = (val & 0xff000000) | (out_r << 16) | (out_g << 8) | out_b;
    }
}

/// Vectorized opacity filter across a slice of ARGB pixels.
pub fn apply_opacity_span(pixels: &mut [u32], factor: f32) {
    let f = factor.clamp(0.0, 1.0);
    let factor_q8 = (f * 256.0).round() as u32;
    for p in pixels.iter_mut() {
        let val = *p;
        let a = (((val >> 24) & 0xff) * factor_q8) >> 8;
        *p = (a.min(255) << 24) | (val & 0x00ffffff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_blend_span() {
        let mut buffer = vec![0x00000000; 16]; // fully black
        let semi_red = Color::rgba(255, 0, 0, 128); // 50% red
        blend_span_normal(&mut buffer, semi_red);

        for &pixel in &buffer {
            let r = (pixel >> 16) & 0xff;
            let g = (pixel >> 8) & 0xff;
            let b = pixel & 0xff;
            // 255 * 128 / 255 ≈ 128
            assert!(
                (127..=129).contains(&r),
                "Red channel should be ~128, got {r}"
            );
            assert_eq!(g, 0);
            assert_eq!(b, 0);
        }
    }

    #[test]
    fn test_simd_grayscale_span() {
        let mut buffer = vec![0xff0000ff; 8]; // fully blue
        apply_grayscale_span(&mut buffer, 1.0);
        for &p in &buffer {
            let r = (p >> 16) & 0xff;
            let g = (p >> 8) & 0xff;
            let b = p & 0xff;
            // 19 * 255 >> 8 ≈ 18-19
            assert!((17..=20).contains(&r));
            assert_eq!(r, g);
            assert_eq!(g, b);
        }
    }

    #[test]
    fn test_simd_invert_span() {
        let mut buffer = vec![0xffff0000; 8]; // red
        apply_invert_span(&mut buffer, 1.0);
        for &p in &buffer {
            let r = (p >> 16) & 0xff;
            let g = (p >> 8) & 0xff;
            let b = p & 0xff;
            assert_eq!(r, 0);
            assert_eq!(g, 255);
            assert_eq!(b, 255);
        }
    }
}
