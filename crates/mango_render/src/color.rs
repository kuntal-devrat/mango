//! Color utilities and named color lookup.

use mango_core::Color;

/// Looks up a named CSS color and returns its RGBA value.
///
/// Supports the CSS Level 4 named colors (a subset for now).
pub fn named_color(name: &str) -> Option<Color> {
    match name.to_lowercase().as_str() {
        "black" => Some(Color::rgb(0, 0, 0)),
        "white" => Some(Color::rgb(255, 255, 255)),
        "red" => Some(Color::rgb(255, 0, 0)),
        "green" => Some(Color::rgb(0, 128, 0)),
        "blue" => Some(Color::rgb(0, 0, 255)),
        "yellow" => Some(Color::rgb(255, 255, 0)),
        "cyan" | "aqua" => Some(Color::rgb(0, 255, 255)),
        "magenta" | "fuchsia" => Some(Color::rgb(255, 0, 255)),
        "orange" => Some(Color::rgb(255, 165, 0)),
        "purple" => Some(Color::rgb(128, 0, 128)),
        "gray" | "grey" => Some(Color::rgb(128, 128, 128)),
        "silver" => Some(Color::rgb(192, 192, 192)),
        "maroon" => Some(Color::rgb(128, 0, 0)),
        "olive" => Some(Color::rgb(128, 128, 0)),
        "lime" => Some(Color::rgb(0, 255, 0)),
        "teal" => Some(Color::rgb(0, 128, 128)),
        "navy" => Some(Color::rgb(0, 0, 128)),
        "darkgray" | "darkgrey" => Some(Color::rgb(169, 169, 169)),
        "lightgray" | "lightgrey" => Some(Color::rgb(211, 211, 211)),
        "darkred" => Some(Color::rgb(139, 0, 0)),
        "darkgreen" => Some(Color::rgb(0, 100, 0)),
        "darkblue" => Some(Color::rgb(0, 0, 139)),
        "coral" => Some(Color::rgb(255, 127, 80)),
        "tomato" => Some(Color::rgb(255, 99, 71)),
        "gold" => Some(Color::rgb(255, 215, 0)),
        "pink" => Some(Color::rgb(255, 192, 203)),
        "brown" => Some(Color::rgb(165, 42, 42)),
        "transparent" => Some(Color::TRANSPARENT),
        _ => None,
    }
}

/// Parses a color string (named, hex, or rgb/rgba) into a [`Color`].
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
