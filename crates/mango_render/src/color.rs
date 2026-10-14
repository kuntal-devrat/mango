//! Color utilities and named color lookup.

use mango_core::Color;

/// Looks up a named CSS color and returns its RGBA value.
///
/// Supports all 148 CSS Color Level 4 named colors (case-insensitive),
/// including `transparent` and `rebeccapurple`.
pub fn named_color(name: &str) -> Option<Color> {
    match name.to_lowercase().as_str() {
        "aliceblue" => Some(Color::rgb(240, 248, 255)),
        "antiquewhite" => Some(Color::rgb(250, 235, 215)),
        "aqua" => Some(Color::rgb(0, 255, 255)),
        "aquamarine" => Some(Color::rgb(127, 255, 212)),
        "azure" => Some(Color::rgb(240, 255, 255)),
        "beige" => Some(Color::rgb(245, 245, 220)),
        "bisque" => Some(Color::rgb(255, 228, 196)),
        "black" => Some(Color::rgb(0, 0, 0)),
        "blanchedalmond" => Some(Color::rgb(255, 235, 205)),
        "blue" => Some(Color::rgb(0, 0, 255)),
        "blueviolet" => Some(Color::rgb(138, 43, 226)),
        "brown" => Some(Color::rgb(165, 42, 42)),
        "burlywood" => Some(Color::rgb(222, 184, 135)),
        "cadetblue" => Some(Color::rgb(95, 158, 160)),
        "chartreuse" => Some(Color::rgb(127, 255, 0)),
        "chocolate" => Some(Color::rgb(210, 105, 30)),
        "coral" => Some(Color::rgb(255, 127, 80)),
        "cornflowerblue" => Some(Color::rgb(100, 149, 237)),
        "cornsilk" => Some(Color::rgb(255, 248, 220)),
        "crimson" => Some(Color::rgb(220, 20, 60)),
        "cyan" => Some(Color::rgb(0, 255, 255)),
        "darkblue" => Some(Color::rgb(0, 0, 139)),
        "darkcyan" => Some(Color::rgb(0, 139, 139)),
        "darkgoldenrod" => Some(Color::rgb(184, 134, 11)),
        "darkgray" => Some(Color::rgb(169, 169, 169)),
        "darkgrey" => Some(Color::rgb(169, 169, 169)),
        "darkgreen" => Some(Color::rgb(0, 100, 0)),
        "darkkhaki" => Some(Color::rgb(189, 183, 107)),
        "darkmagenta" => Some(Color::rgb(139, 0, 139)),
        "darkolivegreen" => Some(Color::rgb(85, 107, 47)),
        "darkorange" => Some(Color::rgb(255, 140, 0)),
        "darkorchid" => Some(Color::rgb(153, 50, 204)),
        "darkred" => Some(Color::rgb(139, 0, 0)),
        "darksalmon" => Some(Color::rgb(233, 150, 122)),
        "darkseagreen" => Some(Color::rgb(143, 188, 143)),
        "darkslateblue" => Some(Color::rgb(72, 61, 139)),
        "darkslategray" => Some(Color::rgb(47, 79, 79)),
        "darkslategrey" => Some(Color::rgb(47, 79, 79)),
        "darkturquoise" => Some(Color::rgb(0, 206, 209)),
        "darkviolet" => Some(Color::rgb(148, 0, 211)),
        "deeppink" => Some(Color::rgb(255, 20, 147)),
        "deepskyblue" => Some(Color::rgb(0, 191, 255)),
        "dimgray" => Some(Color::rgb(105, 105, 105)),
        "dimgrey" => Some(Color::rgb(105, 105, 105)),
        "dodgerblue" => Some(Color::rgb(30, 144, 255)),
        "firebrick" => Some(Color::rgb(178, 34, 34)),
        "floralwhite" => Some(Color::rgb(255, 250, 240)),
        "forestgreen" => Some(Color::rgb(34, 139, 34)),
        "fuchsia" => Some(Color::rgb(255, 0, 255)),
        "gainsboro" => Some(Color::rgb(220, 220, 220)),
        "ghostwhite" => Some(Color::rgb(248, 248, 255)),
        "gold" => Some(Color::rgb(255, 215, 0)),
        "goldenrod" => Some(Color::rgb(218, 165, 32)),
        "gray" => Some(Color::rgb(128, 128, 128)),
        "grey" => Some(Color::rgb(128, 128, 128)),
        "green" => Some(Color::rgb(0, 128, 0)),
        "greenyellow" => Some(Color::rgb(173, 255, 47)),
        "honeydew" => Some(Color::rgb(240, 255, 240)),
        "hotpink" => Some(Color::rgb(255, 105, 180)),
        "indianred" => Some(Color::rgb(205, 92, 92)),
        "indigo" => Some(Color::rgb(75, 0, 130)),
        "ivory" => Some(Color::rgb(255, 255, 240)),
        "khaki" => Some(Color::rgb(240, 230, 140)),
        "lavender" => Some(Color::rgb(230, 230, 250)),
        "lavenderblush" => Some(Color::rgb(255, 240, 245)),
        "lawngreen" => Some(Color::rgb(124, 252, 0)),
        "lemonchiffon" => Some(Color::rgb(255, 250, 205)),
        "lightblue" => Some(Color::rgb(173, 216, 230)),
        "lightcoral" => Some(Color::rgb(240, 128, 128)),
        "lightcyan" => Some(Color::rgb(224, 255, 255)),
        "lightgoldenrodyellow" => Some(Color::rgb(250, 250, 210)),
        "lightgray" => Some(Color::rgb(211, 211, 211)),
        "lightgrey" => Some(Color::rgb(211, 211, 211)),
        "lightgreen" => Some(Color::rgb(144, 238, 144)),
        "lightpink" => Some(Color::rgb(255, 182, 193)),
        "lightsalmon" => Some(Color::rgb(255, 160, 122)),
        "lightseagreen" => Some(Color::rgb(32, 178, 170)),
        "lightskyblue" => Some(Color::rgb(135, 206, 250)),
        "lightslategray" => Some(Color::rgb(119, 136, 153)),
        "lightslategrey" => Some(Color::rgb(119, 136, 153)),
        "lightsteelblue" => Some(Color::rgb(176, 196, 222)),
        "lightyellow" => Some(Color::rgb(255, 255, 224)),
        "lime" => Some(Color::rgb(0, 255, 0)),
        "limegreen" => Some(Color::rgb(50, 205, 50)),
        "linen" => Some(Color::rgb(250, 240, 230)),
        "magenta" => Some(Color::rgb(255, 0, 255)),
        "maroon" => Some(Color::rgb(128, 0, 0)),
        "mediumaquamarine" => Some(Color::rgb(102, 205, 170)),
        "mediumblue" => Some(Color::rgb(0, 0, 205)),
        "mediumorchid" => Some(Color::rgb(186, 85, 211)),
        "mediumpurple" => Some(Color::rgb(147, 112, 219)),
        "mediumseagreen" => Some(Color::rgb(60, 179, 113)),
        "mediumslateblue" => Some(Color::rgb(123, 104, 238)),
        "mediumspringgreen" => Some(Color::rgb(0, 250, 154)),
        "mediumturquoise" => Some(Color::rgb(72, 209, 204)),
        "mediumvioletred" => Some(Color::rgb(199, 21, 133)),
        "midnightblue" => Some(Color::rgb(25, 25, 112)),
        "mintcream" => Some(Color::rgb(245, 255, 250)),
        "mistyrose" => Some(Color::rgb(255, 228, 225)),
        "moccasin" => Some(Color::rgb(255, 228, 181)),
        "navajowhite" => Some(Color::rgb(255, 222, 173)),
        "navy" => Some(Color::rgb(0, 0, 128)),
        "oldlace" => Some(Color::rgb(253, 245, 230)),
        "olive" => Some(Color::rgb(128, 128, 0)),
        "olivedrab" => Some(Color::rgb(107, 142, 35)),
        "orange" => Some(Color::rgb(255, 165, 0)),
        "orangered" => Some(Color::rgb(255, 69, 0)),
        "orchid" => Some(Color::rgb(218, 112, 214)),
        "palegoldenrod" => Some(Color::rgb(238, 232, 170)),
        "palegreen" => Some(Color::rgb(152, 251, 152)),
        "paleturquoise" => Some(Color::rgb(175, 238, 238)),
        "palevioletred" => Some(Color::rgb(219, 112, 147)),
        "papayawhip" => Some(Color::rgb(255, 239, 213)),
        "peachpuff" => Some(Color::rgb(255, 218, 185)),
        "peru" => Some(Color::rgb(205, 133, 63)),
        "pink" => Some(Color::rgb(255, 192, 203)),
        "plum" => Some(Color::rgb(221, 160, 221)),
        "powderblue" => Some(Color::rgb(176, 224, 230)),
        "purple" => Some(Color::rgb(128, 0, 128)),
        "rebeccapurple" => Some(Color::rgb(102, 51, 153)),
        "red" => Some(Color::rgb(255, 0, 0)),
        "rosybrown" => Some(Color::rgb(188, 143, 143)),
        "royalblue" => Some(Color::rgb(65, 105, 225)),
        "saddlebrown" => Some(Color::rgb(139, 69, 19)),
        "salmon" => Some(Color::rgb(250, 128, 114)),
        "sandybrown" => Some(Color::rgb(244, 164, 96)),
        "seagreen" => Some(Color::rgb(46, 139, 87)),
        "seashell" => Some(Color::rgb(255, 245, 238)),
        "sienna" => Some(Color::rgb(160, 82, 45)),
        "silver" => Some(Color::rgb(192, 192, 192)),
        "skyblue" => Some(Color::rgb(135, 206, 235)),
        "slateblue" => Some(Color::rgb(106, 90, 205)),
        "slategray" => Some(Color::rgb(112, 128, 144)),
        "slategrey" => Some(Color::rgb(112, 128, 144)),
        "snow" => Some(Color::rgb(255, 250, 250)),
        "springgreen" => Some(Color::rgb(0, 255, 127)),
        "steelblue" => Some(Color::rgb(70, 130, 180)),
        "tan" => Some(Color::rgb(210, 180, 140)),
        "teal" => Some(Color::rgb(0, 128, 128)),
        "thistle" => Some(Color::rgb(216, 191, 216)),
        "tomato" => Some(Color::rgb(255, 99, 71)),
        "turquoise" => Some(Color::rgb(64, 224, 208)),
        "violet" => Some(Color::rgb(238, 130, 238)),
        "wheat" => Some(Color::rgb(245, 222, 179)),
        "white" => Some(Color::rgb(255, 255, 255)),
        "whitesmoke" => Some(Color::rgb(245, 245, 245)),
        "yellow" => Some(Color::rgb(255, 255, 0)),
        "yellowgreen" => Some(Color::rgb(154, 205, 50)),
        "transparent" => Some(Color::TRANSPARENT),
        _ => None,
    }
}

/// Converts HSL components to RGB.
///
/// `h` in degrees (any value, wrapped to 0..360), `s` and `l` in 0..1.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    if s <= 0.0 {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }
    let mut hue = h % 360.0;
    if hue < 0.0 {
        hue += 360.0;
    }
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (rp, gp, bp) = match hue {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((rp + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((gp + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((bp + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn parse_hue(s: &str) -> Option<f32> {
    let t = s.trim().to_ascii_lowercase();
    if let Some(v) = t.strip_suffix("turn") {
        return v.trim().parse::<f32>().ok().map(|x| x * 360.0);
    }
    if let Some(v) = t.strip_suffix("grad") {
        return v.trim().parse::<f32>().ok().map(|x| x * 0.9);
    }
    if let Some(v) = t.strip_suffix("rad") {
        return v.trim().parse::<f32>().ok().map(|x| x * 180.0 / std::f32::consts::PI);
    }
    if let Some(v) = t.strip_suffix("deg") {
        return v.trim().parse::<f32>().ok();
    }
    t.parse::<f32>().ok()
}

#[allow(dead_code)]
fn parse_percent_or_number_01(s: &str) -> Option<f32> {
    let t = s.trim();
    if let Some(p) = t.strip_suffix('%') {
        let f = p.trim().parse::<f32>().ok()?;
        Some((f / 100.0).clamp(0.0, 1.0))
    } else {
        let f = t.parse::<f32>().ok()?;
        Some(f.clamp(0.0, 1.0))
    }
}

fn parse_sl_fraction(s: &str) -> Option<f32> {
    let t = s.trim();
    if let Some(p) = t.strip_suffix('%') {
        let f = p.trim().parse::<f32>().ok()?;
        Some((f / 100.0).clamp(0.0, 1.0))
    } else {
        // Bare number treated as fraction 0..1 per CSS (clamped).
        let f = t.parse::<f32>().ok()?;
        Some(f.clamp(0.0, 1.0))
    }
}

fn parse_alpha_component(s: &str) -> Option<u8> {
    let t = s.trim();
    if let Some(p) = t.strip_suffix('%') {
        let f = p.trim().parse::<f32>().ok()?;
        Some((f.clamp(0.0, 100.0) / 100.0 * 255.0).round() as u8)
    } else {
        let f = t.parse::<f32>().ok()?;
        Some((f.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

/// Parses `hsl(...)` / `hsla(...)` into a [`Color`].
fn parse_hsl(s: &str) -> Option<Color> {
    let lower = s.trim().to_ascii_lowercase();
    let inner = if lower.starts_with("hsla(") || lower.starts_with("hsl(") {
        let open = lower.find('(')?;
        let close = lower.rfind(')')?;
        if close <= open {
            return None;
        }
        // Use original string slice for numbers (ascii anyway).
        s.trim()[open + 1..close].to_string()
    } else {
        return None;
    };
    // Split off `/ alpha` if present (CSS Color 4 space syntax).
    let (hsl_part, slash_alpha) = if let Some((a, b)) = inner.split_once('/') {
        (a.trim().to_string(), Some(b.trim().to_string()))
    } else {
        (inner, None)
    };
    let parts: Vec<String> = if hsl_part.contains(',') {
        hsl_part.split(',').map(|p| p.trim().to_string()).collect()
    } else {
        hsl_part.split_whitespace().map(|p| p.to_string()).collect()
    };
    if parts.len() < 3 {
        return None;
    }
    let h = parse_hue(&parts[0])?;
    let sat = parse_sl_fraction(&parts[1])?;
    let light = parse_sl_fraction(&parts[2])?;
    let (r, g, b) = hsl_to_rgb(h, sat, light);
    let a = if let Some(a_str) = slash_alpha {
        parse_alpha_component(&a_str)?
    } else if parts.len() >= 4 {
        parse_alpha_component(&parts[3])?
    } else {
        255
    };
    Some(Color::rgba(r, g, b, a))
}

/// Parses a color string (named, hex, rgb/rgba, or hsl/hsla) into a [`Color`].
pub fn parse_color(s: &str) -> Option<Color> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(c) = named_color(trimmed) {
        return Some(c);
    }

    // Hex color: #rgb, #rgba, #rrggbb, #rrggbbaa
    if let Some(hex) = trimmed.strip_prefix('#') {
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

    // hsl(...) or hsla(...)
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("hsl(") || lower.starts_with("hsla(") {
        return parse_hsl(trimmed);
    }

    // rgb(...) or rgba(...)
    if let Some(inner) = trimmed
        .strip_prefix("rgb(")
        .or_else(|| trimmed.strip_prefix("rgba("))
        .and_then(|s| s.strip_suffix(')'))
    {
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

        if parts.len() < 3 {
            return None;
        }

        let parse_comp = |comp: &str| -> Option<u8> {
            let comp = comp.trim();
            if let Some(pct) = comp.strip_suffix('%') {
                let f = pct.parse::<f32>().ok()?;
                Some(((f.clamp(0.0, 100.0) / 100.0) * 255.0).round() as u8)
            } else {
                let f = comp.parse::<f32>().ok()?;
                Some(f.clamp(0.0, 255.0).round() as u8)
            }
        };

        let r = parse_comp(parts[0])?;
        let g = parse_comp(parts[1])?;
        let b = parse_comp(parts[2])?;

        let a = if let Some(a_str) = alpha_part {
            let a_str = a_str.trim().trim_start_matches('/').trim();
            let alpha_f = if let Some(pct) = a_str.strip_suffix('%') {
                pct.parse::<f32>().ok()? / 100.0
            } else {
                a_str.parse::<f32>().ok()?
            };
            (alpha_f.clamp(0.0, 1.0) * 255.0).round() as u8
        } else if parts.len() >= 4 {
            let a_str = parts[3].trim();
            let alpha_f = if let Some(pct) = a_str.strip_suffix('%') {
                pct.parse::<f32>().ok()? / 100.0
            } else {
                a_str.parse::<f32>().ok()?
            };
            (alpha_f.clamp(0.0, 1.0) * 255.0).round() as u8
        } else {
            255
        };

        return Some(Color::rgba(r, g, b, a));
    }

    None
}
