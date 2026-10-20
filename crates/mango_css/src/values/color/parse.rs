use super::named::parse_named_color;
use super::relative::{
    parse_color_mix_inner, parse_relative_color, split_commas_depth0, split_slash_depth0,
};
use super::spaces::{
    a98_rgb_to_srgb, display_p3_to_srgb, hsl_to_rgb, hwb_to_rgb, lab_to_srgb, linear_to_srgb,
    oklab_to_srgb, oklch_to_oklab, prophoto_rgb_to_srgb, rec2020_to_srgb,
};
use mango_core::Color;

/// Attempts to parse a color from a hex string, named color, `rgb()`/`rgba()`,
/// `hsl()`/`hsla()`, `hwb()`, `lab()`, `lch()`, `light-dark()`, `color()`,
/// `oklab()`/`oklch()`, or `color-mix()` string.
///
/// Function names are matched case-insensitively. Unsupported or malformed
/// inputs return `None` so the declaration is dropped per spec, instead of
/// silently defaulting to black.
pub fn parse_color(s: &str) -> Option<Color> {
    let trimmed = s.trim();

    // Hex color: #rgb, #rgba, #rrggbb, #rrggbbaa
    if let Some(hex) = trimmed.strip_prefix('#') {
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        return match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                Some(Color::rgb(r, g, b))
            }
            4 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
                Some(Color::rgba(r, g, b, a))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Color::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Color::rgba(r, g, b, a))
            }
            _ => None,
        };
    }

    // rgb(r, g, b), rgba(r, g, b, a), or modern space-separated rgb(r g b / a).
    // Function names are matched case-insensitively per CSS syntax rules.
    let lower = trimmed.to_ascii_lowercase();
    if let Some(inner) = lower
        .strip_prefix("rgb(")
        .or_else(|| lower.strip_prefix("rgba("))
        .and_then(|s| s.strip_suffix(')'))
    {
        if inner.trim_start().starts_with("from ") {
            if let Some(col) = parse_relative_color("rgb", inner.trim_start()) {
                return Some(col);
            }
        }
        let (rgb_part, alpha_part) = if let Some((rgb_s, a_s)) = inner.split_once('/') {
            (rgb_s.trim(), Some(a_s.trim()))
        } else {
            (inner.trim(), None)
        };

        let parts: Vec<&str> = if rgb_part.contains(',') {
            rgb_part.split(',').map(|p| p.trim()).collect()
        } else {
            rgb_part.split_whitespace().collect()
        };

        let parse_component = |comp: &str| -> Option<u8> {
            let comp = comp.trim();
            if let Some(pct) = comp.strip_suffix('%') {
                let f = pct.parse::<f32>().ok()?;
                Some(((f.clamp(0.0, 100.0) / 100.0) * 255.0).round() as u8)
            } else {
                let f = comp.parse::<f32>().ok()?;
                Some(f.clamp(0.0, 255.0).round() as u8)
            }
        };

        let parse_alpha = |a_str: &str| -> Option<u8> {
            let a_str = a_str.trim().trim_start_matches('/').trim();
            let alpha_f = if let Some(pct) = a_str.strip_suffix('%') {
                pct.parse::<f32>().ok()? / 100.0
            } else {
                a_str.parse::<f32>().ok()?
            };
            Some((alpha_f.clamp(0.0, 1.0) * 255.0).round() as u8)
        };

        if parts.len() == 3 {
            let r = parse_component(parts[0])?;
            let g = parse_component(parts[1])?;
            let b = parse_component(parts[2])?;
            let a = if let Some(a_str) = alpha_part {
                parse_alpha(a_str)?
            } else {
                255
            };
            return Some(Color::rgba(r, g, b, a));
        } else if parts.len() == 4 && alpha_part.is_none() {
            let r = parse_component(parts[0])?;
            let g = parse_component(parts[1])?;
            let b = parse_component(parts[2])?;
            let a = parse_alpha(parts[3])?;
            return Some(Color::rgba(r, g, b, a));
        }
    }

    // hsl(h, s, l), hsla(h, s, l, a), or space-separated hsl(h s l / a)
    if let Some(inner) = lower
        .strip_prefix("hsl(")
        .or_else(|| lower.strip_prefix("hsla("))
        .and_then(|s| s.strip_suffix(')'))
    {
        if inner.trim_start().starts_with("from ") {
            if let Some(col) = parse_relative_color("hsl", inner.trim_start()) {
                return Some(col);
            }
        }
        let (hsl_part, alpha_part) = if let Some((hsl_s, a_s)) = inner.split_once('/') {
            (hsl_s.trim(), Some(a_s.trim()))
        } else {
            (inner.trim(), None)
        };

        let parts: Vec<&str> = if hsl_part.contains(',') {
            hsl_part.split(',').map(|p| p.trim()).collect()
        } else {
            hsl_part.split_whitespace().collect()
        };

        let parse_alpha = |a_str: &str| -> Option<u8> {
            let a_str = a_str.trim().trim_start_matches('/').trim();
            let alpha_f = if let Some(pct) = a_str.strip_suffix('%') {
                pct.parse::<f32>().ok()? / 100.0
            } else {
                a_str.parse::<f32>().ok()?
            };
            Some((alpha_f.clamp(0.0, 1.0) * 255.0).round() as u8)
        };

        if parts.len() == 3 {
            let h = parts[0].trim_end_matches("deg").parse::<f32>().ok()?;
            let s = parts[1].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
            let l = parts[2].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
            let a = if let Some(a_str) = alpha_part {
                parse_alpha(a_str)?
            } else {
                255
            };
            let (r, g, b) = hsl_to_rgb(h, s, l);
            return Some(Color::rgba(r, g, b, a));
        } else if parts.len() == 4 && alpha_part.is_none() {
            let h = parts[0].trim_end_matches("deg").parse::<f32>().ok()?;
            let s = parts[1].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
            let l = parts[2].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
            let a = parse_alpha(parts[3])?;
            let (r, g, b) = hsl_to_rgb(h, s, l);
            return Some(Color::rgba(r, g, b, a));
        }
    }

    // hwb(h w b [/ a]) — CSS Color 4 hue-whiteness-blackness.
    if let Some(inner) = lower.strip_prefix("hwb(").and_then(|s| s.strip_suffix(')')) {
        let (hwb_part, alpha_part) = if let Some((hwb_s, a_s)) = split_slash_depth0(inner) {
            (hwb_s.trim(), Some(a_s.trim()))
        } else {
            (inner.trim(), None)
        };

        let parts: Vec<&str> = if hwb_part.contains(',') {
            hwb_part.split(',').map(|p| p.trim()).collect()
        } else {
            hwb_part.split_whitespace().collect()
        };

        let parse_hue = |s: &str| -> Option<f32> {
            let s = s.trim();
            if let Some(deg) = s.strip_suffix("deg") {
                return deg.trim().parse::<f32>().ok();
            }
            if let Some(rad) = s.strip_suffix("rad") {
                return rad.trim().parse::<f32>().ok().map(|r| r.to_degrees());
            }
            if let Some(turn) = s.strip_suffix("turn") {
                return turn.trim().parse::<f32>().ok().map(|t| t * 360.0);
            }
            if let Some(grad) = s.strip_suffix("grad") {
                return grad.trim().parse::<f32>().ok().map(|g| g * 0.9);
            }
            s.parse::<f32>().ok()
        };
        let parse_wb = |s: &str| -> Option<f32> {
            let s = s.trim();
            if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok().map(|p| p / 100.0)
            } else {
                s.parse::<f32>().ok()
            }
        };
        let parse_alpha = |s: &str| -> Option<u8> {
            let s = s.trim().trim_start_matches('/').trim();
            let f = if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok()? / 100.0
            } else {
                s.parse::<f32>().ok()?
            };
            Some((f.clamp(0.0, 1.0) * 255.0).round() as u8)
        };

        if parts.len() == 3 {
            if let (Some(h), Some(w), Some(b)) =
                (parse_hue(parts[0]), parse_wb(parts[1]), parse_wb(parts[2]))
            {
                let a = if let Some(a_s) = alpha_part {
                    parse_alpha(a_s).unwrap_or(255)
                } else {
                    255
                };
                let (r, g, bl) = hwb_to_rgb(h, w, b);
                return Some(Color::rgba(r, g, bl, a));
            }
        } else if parts.len() == 4 && alpha_part.is_none() {
            if let (Some(h), Some(w), Some(b), Some(a)) = (
                parse_hue(parts[0]),
                parse_wb(parts[1]),
                parse_wb(parts[2]),
                parse_alpha(parts[3]),
            ) {
                let (r, g, bl) = hwb_to_rgb(h, w, b);
                return Some(Color::rgba(r, g, bl, a));
            }
        }
    }

    // lab(l a b [/ alpha]) — CIE Lab with D50 white point.
    if let Some(inner) = lower.strip_prefix("lab(").and_then(|s| s.strip_suffix(')')) {
        let (lab_part, alpha_part) = if let Some((lab_s, a_s)) = split_slash_depth0(inner) {
            (lab_s.trim(), Some(a_s.trim()))
        } else {
            (inner.trim(), None)
        };

        let parts: Vec<&str> = if lab_part.contains(',') {
            lab_part.split(',').map(|p| p.trim()).collect()
        } else {
            lab_part.split_whitespace().collect()
        };

        let parse_l = |s: &str| -> Option<f32> {
            let s = s.trim();
            if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok()
            } else {
                s.parse::<f32>().ok()
            }
        };
        // a/b axes span roughly ±125; percentages are relative to ±125.
        let parse_ab = |s: &str| -> Option<f32> {
            let s = s.trim();
            if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok().map(|p| p * 1.25)
            } else {
                s.parse::<f32>().ok()
            }
        };
        let parse_alpha = |s: &str| -> Option<u8> {
            let s = s.trim().trim_start_matches('/').trim();
            let f = if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok()? / 100.0
            } else {
                s.parse::<f32>().ok()?
            };
            Some((f.clamp(0.0, 1.0) * 255.0).round() as u8)
        };

        if parts.len() == 3 {
            if let (Some(l), Some(a_val), Some(b_val)) =
                (parse_l(parts[0]), parse_ab(parts[1]), parse_ab(parts[2]))
            {
                let a = if let Some(a_s) = alpha_part {
                    parse_alpha(a_s).unwrap_or(255)
                } else {
                    255
                };
                let (r, g, b) = lab_to_srgb(l, a_val, b_val);
                return Some(Color::rgba(r, g, b, a));
            }
        } else if parts.len() == 4 && alpha_part.is_none() {
            if let (Some(l), Some(a_val), Some(b_val), Some(a)) = (
                parse_l(parts[0]),
                parse_ab(parts[1]),
                parse_ab(parts[2]),
                parse_alpha(parts[3]),
            ) {
                let (r, g, b) = lab_to_srgb(l, a_val, b_val);
                return Some(Color::rgba(r, g, b, a));
            }
        }
    }

    // lch(l c h [/ alpha]) — CIE LCh with D50 white point.
    if let Some(inner) = lower.strip_prefix("lch(").and_then(|s| s.strip_suffix(')')) {
        let (lch_part, alpha_part) = if let Some((lch_s, a_s)) = split_slash_depth0(inner) {
            (lch_s.trim(), Some(a_s.trim()))
        } else {
            (inner.trim(), None)
        };

        let parts: Vec<&str> = if lch_part.contains(',') {
            lch_part.split(',').map(|p| p.trim()).collect()
        } else {
            lch_part.split_whitespace().collect()
        };

        let parse_l = |s: &str| -> Option<f32> {
            let s = s.trim();
            if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok()
            } else {
                s.parse::<f32>().ok()
            }
        };
        // Chroma spans roughly 0..150; percentages are relative to 150.
        let parse_c = |s: &str| -> Option<f32> {
            let s = s.trim();
            if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok().map(|p| p * 1.5)
            } else {
                s.parse::<f32>().ok()
            }
        };
        let parse_hue = |s: &str| -> Option<f32> {
            let s = s.trim();
            if s.eq_ignore_ascii_case("none") {
                return Some(0.0);
            }
            if let Some(deg) = s.strip_suffix("deg") {
                return deg.trim().parse::<f32>().ok();
            }
            if let Some(rad) = s.strip_suffix("rad") {
                return rad.trim().parse::<f32>().ok().map(|r| r.to_degrees());
            }
            if let Some(turn) = s.strip_suffix("turn") {
                return turn.trim().parse::<f32>().ok().map(|t| t * 360.0);
            }
            if let Some(grad) = s.strip_suffix("grad") {
                return grad.trim().parse::<f32>().ok().map(|g| g * 0.9);
            }
            s.parse::<f32>().ok()
        };
        let parse_alpha = |s: &str| -> Option<u8> {
            let s = s.trim().trim_start_matches('/').trim();
            let f = if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok()? / 100.0
            } else {
                s.parse::<f32>().ok()?
            };
            Some((f.clamp(0.0, 1.0) * 255.0).round() as u8)
        };

        let to_srgb = |l: f32, c: f32, h: f32, a: u8| -> Color {
            let rad = h.to_radians();
            let (r, g, b) = lab_to_srgb(l, c * rad.cos(), c * rad.sin());
            Color::rgba(r, g, b, a)
        };
        if parts.len() == 3 {
            if let (Some(l), Some(c), Some(h)) =
                (parse_l(parts[0]), parse_c(parts[1]), parse_hue(parts[2]))
            {
                let a = if let Some(a_s) = alpha_part {
                    parse_alpha(a_s).unwrap_or(255)
                } else {
                    255
                };
                return Some(to_srgb(l, c, h, a));
            }
        } else if parts.len() == 4 && alpha_part.is_none() {
            if let (Some(l), Some(c), Some(h), Some(a)) = (
                parse_l(parts[0]),
                parse_c(parts[1]),
                parse_hue(parts[2]),
                parse_alpha(parts[3]),
            ) {
                return Some(to_srgb(l, c, h, a));
            }
        }
    }

    // light-dark(light, dark) — resolves against the used color scheme.
    // Without author color-scheme tracking the used scheme defaults to light,
    // so the first color wins. Unknown/invalid sides invalidate the whole
    // declaration (returns None) instead of defaulting to black.
    if let Some(inner) = lower
        .strip_prefix("light-dark(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts = split_commas_depth0(inner);
        if parts.len() == 2
            && let Some(col) = parse_color(parts[0].trim())
        {
            return Some(col);
        }
    }

    // color(display-p3 r g b [/ a]) or color(srgb r g b [/ a])
    if let Some(inner) = lower
        .strip_prefix("color(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let (col_part, alpha_part) = if let Some((c_s, a_s)) = inner.split_once('/') {
            (c_s.trim(), Some(a_s.trim()))
        } else {
            (inner.trim(), None)
        };

        let parts: Vec<&str> = if col_part.contains(',') {
            col_part.split(',').map(|p| p.trim()).collect()
        } else {
            col_part.split_whitespace().collect()
        };

        if parts.len() >= 4 {
            let space = parts[0].to_ascii_lowercase();
            let parse_f = |s: &str| -> Option<f32> {
                let s = s.trim();
                if let Some(pct) = s.strip_suffix('%') {
                    pct.parse::<f32>().ok().map(|p| p / 100.0)
                } else {
                    s.parse::<f32>().ok()
                }
            };

            if let (Some(c1), Some(c2), Some(c3)) =
                (parse_f(parts[1]), parse_f(parts[2]), parse_f(parts[3]))
            {
                let alpha_f = if let Some(a_s) = alpha_part {
                    parse_f(a_s).unwrap_or(1.0)
                } else if parts.len() >= 5 {
                    parse_f(parts[4]).unwrap_or(1.0)
                } else {
                    1.0
                };
                let a = (alpha_f.clamp(0.0, 1.0) * 255.0).round() as u8;

                if space == "display-p3" {
                    let (r, g, b) = display_p3_to_srgb(c1, c2, c3);
                    return Some(Color::rgba(r, g, b, a));
                } else if space == "rec2020" {
                    let (r, g, b) = rec2020_to_srgb(c1, c2, c3);
                    return Some(Color::rgba(r, g, b, a));
                } else if space == "a98-rgb" {
                    let (r, g, b) = a98_rgb_to_srgb(c1, c2, c3);
                    return Some(Color::rgba(r, g, b, a));
                } else if space == "prophoto-rgb" {
                    let (r, g, b) = prophoto_rgb_to_srgb(c1, c2, c3);
                    return Some(Color::rgba(r, g, b, a));
                } else if space == "srgb" {
                    let r = (c1.clamp(0.0, 1.0) * 255.0).round() as u8;
                    let g = (c2.clamp(0.0, 1.0) * 255.0).round() as u8;
                    let b = (c3.clamp(0.0, 1.0) * 255.0).round() as u8;
                    return Some(Color::rgba(r, g, b, a));
                } else if space == "srgb-linear" {
                    let r = (linear_to_srgb(c1).clamp(0.0, 1.0) * 255.0).round() as u8;
                    let g = (linear_to_srgb(c2).clamp(0.0, 1.0) * 255.0).round() as u8;
                    let b = (linear_to_srgb(c3).clamp(0.0, 1.0) * 255.0).round() as u8;
                    return Some(Color::rgba(r, g, b, a));
                }
            }
        }
    }

    // oklab(L a b [/ alpha])
    if let Some(inner) = lower
        .strip_prefix("oklab(")
        .and_then(|s| s.strip_suffix(')'))
    {
        if inner.trim_start().starts_with("from ") {
            if let Some(col) = parse_relative_color("oklab", inner.trim_start()) {
                return Some(col);
            }
        }
        let (lab_part, alpha_part) = if let Some((lab_s, a_s)) = inner.split_once('/') {
            (lab_s.trim(), Some(a_s.trim()))
        } else {
            (inner.trim(), None)
        };

        let parts: Vec<&str> = if lab_part.contains(',') {
            lab_part.split(',').map(|p| p.trim()).collect()
        } else {
            lab_part.split_whitespace().collect()
        };

        let parse_l = |s: &str| -> Option<f32> {
            let s = s.trim();
            if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok().map(|p| p / 100.0)
            } else {
                s.parse::<f32>().ok()
            }
        };
        let parse_ab = |s: &str| -> Option<f32> {
            let s = s.trim();
            if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok().map(|p| (p / 100.0) * 0.4)
            } else {
                s.parse::<f32>().ok()
            }
        };
        let parse_alpha = |s: &str| -> Option<u8> {
            let s = s.trim().trim_start_matches('/').trim();
            let f = if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok()? / 100.0
            } else {
                s.parse::<f32>().ok()?
            };
            Some((f.clamp(0.0, 1.0) * 255.0).round() as u8)
        };

        if parts.len() == 3 {
            if let (Some(l), Some(a_val), Some(b_val)) =
                (parse_l(parts[0]), parse_ab(parts[1]), parse_ab(parts[2]))
            {
                let a = if let Some(a_s) = alpha_part {
                    parse_alpha(a_s).unwrap_or(255)
                } else {
                    255
                };
                let (r, g, b) = oklab_to_srgb(l, a_val, b_val);
                return Some(Color::rgba(r, g, b, a));
            }
        } else if parts.len() == 4
            && alpha_part.is_none()
            && let (Some(l), Some(a_val), Some(b_val), Some(a)) = (
                parse_l(parts[0]),
                parse_ab(parts[1]),
                parse_ab(parts[2]),
                parse_alpha(parts[3]),
            )
        {
            let (r, g, b) = oklab_to_srgb(l, a_val, b_val);
            return Some(Color::rgba(r, g, b, a));
        }
    }

    // oklch(L C h [/ alpha])
    if let Some(inner) = lower
        .strip_prefix("oklch(")
        .and_then(|s| s.strip_suffix(')'))
    {
        if inner.trim_start().starts_with("from ") {
            if let Some(col) = parse_relative_color("oklch", inner.trim_start()) {
                return Some(col);
            }
        }
        let (lch_part, alpha_part) = if let Some((lch_s, a_s)) = inner.split_once('/') {
            (lch_s.trim(), Some(a_s.trim()))
        } else {
            (inner.trim(), None)
        };

        let parts: Vec<&str> = if lch_part.contains(',') {
            lch_part.split(',').map(|p| p.trim()).collect()
        } else {
            lch_part.split_whitespace().collect()
        };

        let parse_l = |s: &str| -> Option<f32> {
            let s = s.trim();
            if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok().map(|p| p / 100.0)
            } else {
                s.parse::<f32>().ok()
            }
        };
        let parse_c = |s: &str| -> Option<f32> {
            let s = s.trim();
            if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok().map(|p| (p / 100.0) * 0.4)
            } else {
                s.parse::<f32>().ok()
            }
        };
        let parse_hue = |s: &str| -> Option<f32> {
            let s = s.trim();
            if s.eq_ignore_ascii_case("none") {
                return Some(0.0);
            }
            if let Some(deg) = s.strip_suffix("deg") {
                return deg.trim().parse::<f32>().ok();
            }
            if let Some(rad) = s.strip_suffix("rad") {
                return rad.trim().parse::<f32>().ok().map(|r| r.to_degrees());
            }
            if let Some(turn) = s.strip_suffix("turn") {
                return turn.trim().parse::<f32>().ok().map(|t| t * 360.0);
            }
            if let Some(grad) = s.strip_suffix("grad") {
                return grad.trim().parse::<f32>().ok().map(|g| g * 0.9);
            }
            s.parse::<f32>().ok()
        };
        let parse_alpha = |s: &str| -> Option<u8> {
            let s = s.trim().trim_start_matches('/').trim();
            let f = if let Some(pct) = s.strip_suffix('%') {
                pct.parse::<f32>().ok()? / 100.0
            } else {
                s.parse::<f32>().ok()?
            };
            Some((f.clamp(0.0, 1.0) * 255.0).round() as u8)
        };

        if parts.len() == 3 {
            if let (Some(l), Some(c), Some(h)) =
                (parse_l(parts[0]), parse_c(parts[1]), parse_hue(parts[2]))
            {
                let a = if let Some(a_s) = alpha_part {
                    parse_alpha(a_s).unwrap_or(255)
                } else {
                    255
                };
                let (ok_l, ok_a, ok_b) = oklch_to_oklab(l, c, h);
                let (r, g, b) = oklab_to_srgb(ok_l, ok_a, ok_b);
                return Some(Color::rgba(r, g, b, a));
            }
        } else if parts.len() == 4
            && alpha_part.is_none()
            && let (Some(l), Some(c), Some(h), Some(a)) = (
                parse_l(parts[0]),
                parse_c(parts[1]),
                parse_hue(parts[2]),
                parse_alpha(parts[3]),
            )
        {
            let (ok_l, ok_a, ok_b) = oklch_to_oklab(l, c, h);
            let (r, g, b) = oklab_to_srgb(ok_l, ok_a, ok_b);
            return Some(Color::rgba(r, g, b, a));
        }
    }

    // color-mix(in <color-space>, <color1> [p1], <color2> [p2])
    if let Some(inner) = lower
        .strip_prefix("color-mix(")
        .and_then(|s| s.strip_suffix(')'))
        && let Some(col) = parse_color_mix_inner(inner)
    {
        return Some(col);
    }

    // Full CSS 148 standard named colors (CSS Color Module Level 3/4)
    parse_named_color(trimmed)
}
