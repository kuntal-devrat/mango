use super::spaces::{
    hsl_to_rgb, linear_to_srgb, oklab_to_oklch, oklab_to_srgb, oklch_to_oklab, srgb_to_hsl,
    srgb_to_linear, srgb_to_oklab,
};
use mango_core::Color;

pub(crate) fn split_commas_depth0(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

pub(crate) fn split_slash_depth0(s: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '/' if depth == 0 => {
                let left = &s[..i];
                let right = &s[i + 1..];
                return Some((left, right));
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn parse_color_mix_inner(inner: &str) -> Option<Color> {
    let parts = split_commas_depth0(inner);
    if parts.len() < 3 {
        return None;
    }
    let first = parts[0].trim();
    let space = if let Some(sp) = first.strip_prefix("in ") {
        sp.trim().to_ascii_lowercase()
    } else if let Some(sp) = first.strip_prefix("IN ") {
        sp.trim().to_ascii_lowercase()
    } else {
        first.to_ascii_lowercase()
    };

    let parse_color_and_pct = |s: &str| -> Option<(Color, Option<f32>)> {
        let s = s.trim();
        if let Some((col_str, pct_str)) = s.rsplit_once(char::is_whitespace)
            && let Some(pct) = pct_str.strip_suffix('%')
            && let Ok(val) = pct.trim().parse::<f32>()
            && let Some(c) = super::parse::parse_color(col_str)
        {
            return Some((c, Some(val / 100.0)));
        }
        super::parse::parse_color(s).map(|c| (c, None))
    };

    let (c1, p1_opt) = parse_color_and_pct(&parts[1])?;
    let (c2, p2_opt) = parse_color_and_pct(&parts[2])?;

    let (w1, w2, a_mult) = match (p1_opt, p2_opt) {
        (Some(p1), Some(p2)) => {
            let sum = p1 + p2;
            if sum <= 0.0 {
                (0.5, 0.5, 1.0)
            } else {
                (p1 / sum, p2 / sum, sum.min(1.0))
            }
        }
        (Some(p1), None) => {
            let w1 = p1.clamp(0.0, 1.0);
            (w1, 1.0 - w1, 1.0)
        }
        (None, Some(p2)) => {
            let w2 = p2.clamp(0.0, 1.0);
            (1.0 - w2, w2, 1.0)
        }
        (None, None) => (0.5, 0.5, 1.0),
    };

    let alpha = ((c1.a as f32 * w1 + c2.a as f32 * w2) * a_mult)
        .clamp(0.0, 255.0)
        .round() as u8;

    if space == "oklab" {
        let (l1, a1, b1) = srgb_to_oklab(c1.r, c1.g, c1.b);
        let (l2, a2, b2) = srgb_to_oklab(c2.r, c2.g, c2.b);
        let l = l1 * w1 + l2 * w2;
        let a_val = a1 * w1 + a2 * w2;
        let b_val = b1 * w1 + b2 * w2;
        let (r, g, b) = oklab_to_srgb(l, a_val, b_val);
        Some(Color::rgba(r, g, b, alpha))
    } else if space == "oklch" {
        let (l1, a1, b1) = srgb_to_oklab(c1.r, c1.g, c1.b);
        let (l2, a2, b2) = srgb_to_oklab(c2.r, c2.g, c2.b);
        let (ok_l1, c_val1, h1) = oklab_to_oklch(l1, a1, b1);
        let (ok_l2, c_val2, h2) = oklab_to_oklch(l2, a2, b2);
        let l = ok_l1 * w1 + ok_l2 * w2;
        let c = c_val1 * w1 + c_val2 * w2;
        let mut diff = (h2 - h1).rem_euclid(360.0);
        if diff > 180.0 {
            diff -= 360.0;
        }
        let h = (h1 + diff * w2).rem_euclid(360.0);
        let (lab_l, lab_a, lab_b) = oklch_to_oklab(l, c, h);
        let (r, g, b) = oklab_to_srgb(lab_l, lab_a, lab_b);
        Some(Color::rgba(r, g, b, alpha))
    } else {
        // srgb, display-p3, linear srgb interpolation
        let r_lin =
            srgb_to_linear(c1.r as f32 / 255.0) * w1 + srgb_to_linear(c2.r as f32 / 255.0) * w2;
        let g_lin =
            srgb_to_linear(c1.g as f32 / 255.0) * w1 + srgb_to_linear(c2.g as f32 / 255.0) * w2;
        let b_lin =
            srgb_to_linear(c1.b as f32 / 255.0) * w1 + srgb_to_linear(c2.b as f32 / 255.0) * w2;
        let r = (linear_to_srgb(r_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
        let g = (linear_to_srgb(g_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
        let b = (linear_to_srgb(b_lin).clamp(0.0, 1.0) * 255.0).round() as u8;
        Some(Color::rgba(r, g, b, alpha))
    }
}

pub(crate) fn evaluate_channel_expr(expr: &str, vars: &[(&str, f32)]) -> Option<f32> {
    let mut s = expr.trim().to_ascii_lowercase();
    if let Some(inner) = s.strip_prefix("calc(").and_then(|x| x.strip_suffix(')')) {
        s = inner.trim().to_string();
    }
    for &(var, val) in vars {
        if s == var {
            return Some(val);
        }
        if s.contains(var) {
            s = s.replace(var, &val.to_string());
        }
    }
    if let Some((left, right)) = s.split_once('+') {
        let l = evaluate_channel_expr(left, vars)?;
        let r = evaluate_channel_expr(right, vars)?;
        return Some(l + r);
    }
    if let Some((left, right)) = s.split_once('-') {
        if !left.trim().is_empty() {
            let l = evaluate_channel_expr(left, vars)?;
            let r = evaluate_channel_expr(right, vars)?;
            return Some(l - r);
        }
    }
    if let Some((left, right)) = s.split_once('*') {
        let l = evaluate_channel_expr(left, vars)?;
        let r = evaluate_channel_expr(right, vars)?;
        return Some(l * r);
    }
    if let Some((left, right)) = s.split_once('/') {
        let l = evaluate_channel_expr(left, vars)?;
        let r = evaluate_channel_expr(right, vars)?;
        if r != 0.0 {
            return Some(l / r);
        }
    }
    if let Some(pct) = s.strip_suffix('%') {
        pct.trim().parse::<f32>().ok()
    } else if let Some(deg) = s.strip_suffix("deg") {
        deg.trim().parse::<f32>().ok()
    } else {
        s.trim().parse::<f32>().ok()
    }
}

pub(crate) fn split_channels_respecting_parens(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let bytes = s.as_bytes();
    let is_comma = s.contains(',');

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'(' {
            depth += 1;
        } else if b == b')' {
            if depth > 0 {
                depth -= 1;
            }
        } else if depth == 0 {
            if is_comma {
                if b == b',' {
                    let part = s[start..i].trim();
                    if !part.is_empty() {
                        parts.push(part);
                    }
                    start = i + 1;
                }
            } else if b.is_ascii_whitespace() {
                let part = s[start..i].trim();
                if !part.is_empty() {
                    parts.push(part);
                }
                start = i + 1;
            }
        }
    }
    let last = s[start..].trim();
    if !last.is_empty() {
        parts.push(last);
    }
    parts
}

pub(crate) fn parse_relative_color(fn_name: &str, inner: &str) -> Option<Color> {
    let after_from = inner.strip_prefix("from ")?.trim();
    let (origin_str, rest) = if after_from.contains('(') {
        let first_word = after_from.split_whitespace().next()?;
        if first_word.contains('(') {
            let mut depth = 0;
            let mut close_idx = 0;
            for (i, ch) in after_from.char_indices() {
                if ch == '(' {
                    depth += 1;
                } else if ch == ')' {
                    depth -= 1;
                    if depth == 0 {
                        close_idx = i;
                        break;
                    }
                }
            }
            if close_idx > 0 {
                (
                    &after_from[..=close_idx],
                    after_from[close_idx + 1..].trim(),
                )
            } else {
                return None;
            }
        } else {
            let (first, remainder) = after_from.split_once(char::is_whitespace)?;
            (first.trim(), remainder.trim())
        }
    } else {
        let (first, remainder) = after_from.split_once(char::is_whitespace)?;
        (first.trim(), remainder.trim())
    };

    let origin = super::parse::parse_color(origin_str)?;
    let (channels_part, alpha_part) = if let Some((c_s, a_s)) = rest.split_once('/') {
        (c_s.trim(), Some(a_s.trim()))
    } else {
        (rest.trim(), None)
    };

    let parts = split_channels_respecting_parens(channels_part);
    if parts.len() < 3 {
        return None;
    }

    let parse_alpha = |expr: &str, vars: &[(&str, f32)]| -> Option<u8> {
        let val = evaluate_channel_expr(expr, vars)?;
        let val = if expr.trim().ends_with('%') || val > 1.0 {
            val / 100.0
        } else {
            val
        };
        Some((val.clamp(0.0, 1.0) * 255.0).round() as u8)
    };

    match fn_name {
        "rgb" | "rgba" => {
            let vars = [
                ("r", origin.r as f32),
                ("g", origin.g as f32),
                ("b", origin.b as f32),
                ("alpha", origin.a as f32 / 255.0),
            ];
            let parse_rgb_chan = |expr: &str| -> Option<u8> {
                let val = evaluate_channel_expr(expr, &vars)?;
                let val = if expr.trim().ends_with('%') {
                    (val / 100.0) * 255.0
                } else {
                    val
                };
                Some(val.clamp(0.0, 255.0).round() as u8)
            };
            let r = parse_rgb_chan(parts[0])?;
            let g = parse_rgb_chan(parts[1])?;
            let b = parse_rgb_chan(parts[2])?;
            let a = if let Some(a_s) = alpha_part {
                parse_alpha(a_s, &vars)?
            } else {
                origin.a
            };
            Some(Color::rgba(r, g, b, a))
        }
        "hsl" | "hsla" => {
            let (orig_h, orig_s, orig_l) = srgb_to_hsl(origin.r, origin.g, origin.b);
            let vars = [
                ("h", orig_h),
                ("s", orig_s * 100.0),
                ("l", orig_l * 100.0),
                ("alpha", origin.a as f32 / 255.0),
            ];
            let parse_hsl_chan = |expr: &str| -> Option<f32> {
                let val = evaluate_channel_expr(expr, &vars)?;
                let normalized = if expr.trim().ends_with('%') || val > 1.0 {
                    val / 100.0
                } else {
                    val
                };
                Some(normalized.clamp(0.0, 1.0))
            };
            let h = evaluate_channel_expr(parts[0], &vars)?;
            let s = parse_hsl_chan(parts[1])?;
            let l = parse_hsl_chan(parts[2])?;
            let a = if let Some(a_s) = alpha_part {
                parse_alpha(a_s, &vars)?
            } else {
                origin.a
            };
            let (r, g, b) = hsl_to_rgb(h, s, l);
            Some(Color::rgba(r, g, b, a))
        }
        "oklab" => {
            let (orig_l, orig_a, orig_b) = srgb_to_oklab(origin.r, origin.g, origin.b);
            let vars = [
                ("l", orig_l),
                ("a", orig_a),
                ("b", orig_b),
                ("alpha", origin.a as f32 / 255.0),
            ];
            let parse_ok_l = |expr: &str| -> Option<f32> {
                let val = evaluate_channel_expr(expr, &vars)?;
                let normalized = if expr.trim().ends_with('%') || val > 1.0 {
                    val / 100.0
                } else {
                    val
                };
                Some(normalized.clamp(0.0, 1.0))
            };
            let l = parse_ok_l(parts[0])?;
            let a_val = evaluate_channel_expr(parts[1], &vars)?;
            let b_val = evaluate_channel_expr(parts[2], &vars)?;
            let a = if let Some(a_s) = alpha_part {
                parse_alpha(a_s, &vars)?
            } else {
                origin.a
            };
            let (r, g, b) = oklab_to_srgb(l, a_val, b_val);
            Some(Color::rgba(r, g, b, a))
        }
        "oklch" => {
            let (orig_l, orig_a, orig_b) = srgb_to_oklab(origin.r, origin.g, origin.b);
            let (orig_l, orig_c, orig_h) = oklab_to_oklch(orig_l, orig_a, orig_b);
            let vars = [
                ("l", orig_l),
                ("c", orig_c),
                ("h", orig_h),
                ("alpha", origin.a as f32 / 255.0),
            ];
            let parse_ok_l = |expr: &str| -> Option<f32> {
                let val = evaluate_channel_expr(expr, &vars)?;
                let normalized = if expr.trim().ends_with('%') || val > 1.0 {
                    val / 100.0
                } else {
                    val
                };
                Some(normalized.clamp(0.0, 1.0))
            };
            let l = parse_ok_l(parts[0])?;
            let c = evaluate_channel_expr(parts[1], &vars)?;
            let h = evaluate_channel_expr(parts[2], &vars)?;
            let a = if let Some(a_s) = alpha_part {
                parse_alpha(a_s, &vars)?
            } else {
                origin.a
            };
            let (ok_l, ok_a, ok_b) = oklch_to_oklab(l, c, h);
            let (r, g, b) = oklab_to_srgb(ok_l, ok_a, ok_b);
            Some(Color::rgba(r, g, b, a))
        }
        _ => None,
    }
}
