/// Converts HSL color values to RGB (0..255).
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r_prime, g_prime, b_prime) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (
        ((r_prime + m) * 255.0).round() as u8,
        ((g_prime + m) * 255.0).round() as u8,
        ((b_prime + m) * 255.0).round() as u8,
    )
}

/// Converts sRGB bytes `(r, g, b)` to HSL coordinates `(h_deg, s_01, l_01)`.
pub fn srgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r_f = r as f32 / 255.0;
    let g_f = g as f32 / 255.0;
    let b_f = b as f32 / 255.0;
    let max = r_f.max(g_f).max(b_f);
    let min = r_f.min(g_f).min(b_f);
    let delta = max - min;
    let l = (max + min) / 2.0;
    if delta == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = if l < 0.5 {
        delta / (max + min)
    } else {
        delta / (2.0 - max - min)
    };
    let mut h = if max == r_f {
        (g_f - b_f) / delta + (if g_f < b_f { 6.0 } else { 0.0 })
    } else if max == g_f {
        (b_f - r_f) / delta + 2.0
    } else {
        (r_f - g_f) / delta + 4.0
    };
    h *= 60.0;
    (h, s, l)
}

/// Converts HWB values to RGB (0..255).
///
/// `h` is a hue angle in degrees, `w`/`b` are whiteness/blackness in `0.0..=1.0`.
/// Follows CSS Color 4: the pure hue is mixed toward white by `w`, with `w + b`
/// renormalized when their sum exceeds 1.
pub fn hwb_to_rgb(h: f32, w: f32, b: f32) -> (u8, u8, u8) {
    let (w, b) = if w + b > 1.0 {
        (w / (w + b), b / (w + b))
    } else {
        (w.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
    };
    let (pr, pg, pb) = hsl_to_rgb(h, 1.0, 0.5);
    let mix = |c: u8| {
        ((c as f32 / 255.0) * (1.0 - w - b) + w)
            .clamp(0.0, 1.0)
            .mul_add(255.0, 0.0)
            .round() as u8
    };
    (mix(pr), mix(pg), mix(pb))
}

/// Converts CIE Lab (`L` in `0.0..=100.0`, D50 white point) to sRGB bytes.
pub fn lab_to_srgb(l: f32, a: f32, b: f32) -> (u8, u8, u8) {
    let l = l.clamp(0.0, 100.0);
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    // Inverse CIE companding.
    let inv = |t: f32| {
        if t.powi(3) > 0.008856 {
            t.powi(3)
        } else {
            (t - 16.0 / 116.0) / 7.787
        }
    };
    // D50 tristimulus (0..100).
    let x = 96.422 * inv(fx);
    let y = 100.0 * inv(fy);
    let z = 82.521 * inv(fz);
    // Bradford-adapted D50 -> D65 linear sRGB matrix (combined).
    let x = x / 100.0;
    let y = y / 100.0;
    let z = z / 100.0;
    let r_lin = 3.134_186_9 * x - 1.617_209_0 * y - 0.490_694_2 * z;
    let g_lin = -0.978_748_4 * x + 1.916_130_1 * y + 0.033_433_0 * z;
    let b_lin = 0.071_963_9 * x - 0.228_993_9 * y + 1.405_242_7 * z;
    (
        (linear_to_srgb(r_lin).clamp(0.0, 1.0) * 255.0).round() as u8,
        (linear_to_srgb(g_lin).clamp(0.0, 1.0) * 255.0).round() as u8,
        (linear_to_srgb(b_lin).clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

/// Converts an sRGB channel value in `0.0..=1.0` to linear light.
#[inline]
pub fn srgb_to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts a linear light value in `0.0..=1.0` to sRGB gamma space.
#[inline]
pub fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Converts Display P3 coordinates `(r, g, b)` (in `0.0..=1.0`) to sRGB `(r, g, b)` bytes.
pub fn display_p3_to_srgb(r: f32, g: f32, b: f32) -> (u8, u8, u8) {
    let r_lin = srgb_to_linear(r);
    let g_lin = srgb_to_linear(g);
    let b_lin = srgb_to_linear(b);

    let r_srgb_lin = 1.224_940_2 * r_lin - 0.224_940_18 * g_lin + 0.0 * b_lin;
    let g_srgb_lin = -0.042056916 * r_lin + 1.042_056_9 * g_lin + 0.0 * b_lin;
    let b_srgb_lin = -0.019637554 * r_lin - 0.078_636_04 * g_lin + 1.098_273_6 * b_lin;

    let r_out = (linear_to_srgb(r_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let g_out = (linear_to_srgb(g_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let b_out = (linear_to_srgb(b_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;

    (r_out, g_out, b_out)
}

/// Converts Rec. 2020 coordinates `(r, g, b)` (in `0.0..=1.0`) to sRGB `(r, g, b)` bytes.
pub fn rec2020_to_srgb(r: f32, g: f32, b: f32) -> (u8, u8, u8) {
    let r_lin = r.clamp(0.0, 1.0);
    let g_lin = g.clamp(0.0, 1.0);
    let b_lin = b.clamp(0.0, 1.0);

    let r_srgb_lin = 1.660_491 * r_lin - 0.587_641_1 * g_lin - 0.072_849_9 * b_lin;
    let g_srgb_lin = -0.124_550_5 * r_lin + 1.132_899_9 * g_lin - 0.008_349_4 * b_lin;
    let b_srgb_lin = -0.018_150_8 * r_lin - 0.100_578_9 * g_lin + 1.118_729_7 * b_lin;

    let r_out = (linear_to_srgb(r_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let g_out = (linear_to_srgb(g_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let b_out = (linear_to_srgb(b_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;

    (r_out, g_out, b_out)
}

/// Converts Adobe 1998 RGB coordinates `(r, g, b)` (in `0.0..=1.0`) to sRGB `(r, g, b)` bytes.
pub fn a98_rgb_to_srgb(r: f32, g: f32, b: f32) -> (u8, u8, u8) {
    let r_lin = r.clamp(0.0, 1.0).powf(2.199_218_8);
    let g_lin = g.clamp(0.0, 1.0).powf(2.199_218_8);
    let b_lin = b.clamp(0.0, 1.0).powf(2.199_218_8);

    let r_srgb_lin = 1.398_283_1 * r_lin - 0.398_283_1 * g_lin;
    let g_srgb_lin = g_lin;
    let b_srgb_lin = -0.042_928_3 * g_lin + 1.042_928_3 * b_lin;

    let r_out = (linear_to_srgb(r_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let g_out = (linear_to_srgb(g_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let b_out = (linear_to_srgb(b_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;

    (r_out, g_out, b_out)
}

/// Converts ProPhoto RGB coordinates `(r, g, b)` (in `0.0..=1.0`) to sRGB `(r, g, b)` bytes.
pub fn prophoto_rgb_to_srgb(r: f32, g: f32, b: f32) -> (u8, u8, u8) {
    let r_lin = r.clamp(0.0, 1.0).powf(1.8);
    let g_lin = g.clamp(0.0, 1.0).powf(1.8);
    let b_lin = b.clamp(0.0, 1.0).powf(1.8);

    let r_srgb_lin = 2.034_080_5 * r_lin - 0.727_215 * g_lin - 0.306_865_5 * b_lin;
    let g_srgb_lin = -0.228_822_6 * r_lin + 1.231_728_6 * g_lin - 0.002_906 * b_lin;
    let b_srgb_lin = -0.008_565_3 * r_lin - 0.153_273_5 * g_lin + 1.161_838_8 * b_lin;

    let r_out = (linear_to_srgb(r_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let g_out = (linear_to_srgb(g_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let b_out = (linear_to_srgb(b_srgb_lin).clamp(0.0, 1.0) * 255.0).round() as u8;

    (r_out, g_out, b_out)
}

/// Converts Oklab coordinates `(L, a, b)` to standard sRGB `(r, g, b)` bytes.
pub fn oklab_to_srgb(l: f32, a: f32, b: f32) -> (u8, u8, u8) {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);

    let r_lin = 4.076_743_6 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_;
    let g_lin = -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_;
    let b_lin = -0.0041960863 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_;

    let r = (linear_to_srgb(r_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (linear_to_srgb(g_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (linear_to_srgb(b_lin).clamp(0.0, 1.0) * 255.0).round() as u8;

    (r, g, b)
}

/// Converts sRGB bytes `(r, g, b)` to Oklab coordinates `(L, a, b)`.
pub fn srgb_to_oklab(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r_lin = srgb_to_linear(r as f32 / 255.0);
    let g_lin = srgb_to_linear(g as f32 / 255.0);
    let b_lin = srgb_to_linear(b as f32 / 255.0);

    let l = (0.412_221_46 * r_lin + 0.536_332_55 * g_lin + 0.051_445_995 * b_lin).cbrt();
    let m = (0.211_903_5 * r_lin + 0.680_699_5 * g_lin + 0.107_396_96 * b_lin).cbrt();
    let s = (0.088_302_46 * r_lin + 0.281_718_85 * g_lin + 0.629_978_7 * b_lin).cbrt();

    let l_out = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let a_out = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let b_out = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;

    (l_out, a_out, b_out)
}

/// Converts Oklch coordinates `(L, C, h_deg)` to Oklab `(L, a, b)`.
pub fn oklch_to_oklab(l: f32, c: f32, h_deg: f32) -> (f32, f32, f32) {
    let h_rad = h_deg.to_radians();
    let a = c * h_rad.cos();
    let b = c * h_rad.sin();
    (l, a, b)
}

/// Converts Oklab coordinates `(L, a, b)` to Oklch `(L, C, h_deg)`.
pub fn oklab_to_oklch(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    let c = (a * a + b * b).sqrt();
    let mut h = b.atan2(a).to_degrees();
    if h < 0.0 {
        h += 360.0;
    }
    (l, c, h)
}
