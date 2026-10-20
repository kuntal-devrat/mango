//! 32-bit RGBA Color type with hex parsing, luminance, and contrast calculations.

use std::fmt;

/// An RGBA color with 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red channel (0–255).
    pub r: u8,
    /// Green channel (0–255).
    pub g: u8,
    /// Blue channel (0–255).
    pub b: u8,
    /// Alpha channel (0 = transparent, 255 = opaque).
    pub a: u8,
}

impl Color {
    /// Opaque black `(0, 0, 0, 255)`.
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    /// Opaque white `(255, 255, 255, 255)`.
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    /// Opaque red `(255, 0, 0, 255)`.
    pub const RED: Color = Color::rgb(255, 0, 0);
    /// Opaque green `(0, 128, 0, 255)` — CSS "green".
    pub const GREEN: Color = Color::rgb(0, 128, 0);
    /// Opaque blue `(0, 0, 255, 255)`.
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    /// Fully transparent black `(0, 0, 0, 0)`.
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);

    /// Mango brand orange.
    pub const MANGO_ORANGE: Color = Color::rgb(255, 161, 54);
    /// Mango brand dark background.
    pub const MANGO_DARK: Color = Color::rgb(30, 30, 30);
    /// Mango brand light background.
    pub const MANGO_LIGHT: Color = Color::rgb(245, 245, 240);

    /// Creates an opaque color from RGB channels.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Creates a color from RGBA channels.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Creates a color from a 24-bit hex value (e.g., `0xFF9933`), with alpha set to 255.
    pub const fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
            a: 255,
        }
    }

    /// Creates an opaque color from a 24-bit integer `0x00RRGGBB`. Alias for [`from_hex`](Self::from_hex).
    #[inline]
    pub const fn from_rgb_u32(val: u32) -> Self {
        Self::from_hex(val)
    }

    /// Creates a color from a 32-bit packed integer `0xAARRGGBB`.
    #[inline]
    pub const fn from_argb_u32(val: u32) -> Self {
        Self {
            a: ((val >> 24) & 0xFF) as u8,
            r: ((val >> 16) & 0xFF) as u8,
            g: ((val >> 8) & 0xFF) as u8,
            b: (val & 0xFF) as u8,
        }
    }

    /// Creates a color from a 32-bit packed integer `0xRRGGBBAA`.
    #[inline]
    pub const fn from_rgba_u32(val: u32) -> Self {
        Self {
            r: ((val >> 24) & 0xFF) as u8,
            g: ((val >> 16) & 0xFF) as u8,
            b: ((val >> 8) & 0xFF) as u8,
            a: (val & 0xFF) as u8,
        }
    }

    /// Packs into a `u32` as `0xAARRGGBB` (suitable for framebuffer pixels).
    #[inline]
    pub fn to_argb_u32(self) -> u32 {
        (self.a as u32) << 24 | (self.r as u32) << 16 | (self.g as u32) << 8 | (self.b as u32)
    }

    /// Packs into a `u32` as `0x00RRGGBB` (suitable for softbuffer).
    #[inline]
    pub fn to_rgb_u32(self) -> u32 {
        (self.r as u32) << 16 | (self.g as u32) << 8 | (self.b as u32)
    }

    /// Packs into a `u32` as `0xRRGGBBAA`.
    #[inline]
    pub fn to_rgba_u32(self) -> u32 {
        (self.r as u32) << 24 | (self.g as u32) << 16 | (self.b as u32) << 8 | (self.a as u32)
    }

    /// Returns color components as an RGBA byte array `[r, g, b, a]`.
    #[inline]
    pub fn to_rgba_bytes(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Returns color components as a BGRA byte array `[b, g, r, a]` (common framebuffer layout).
    #[inline]
    pub fn to_bgra_bytes(self) -> [u8; 4] {
        [self.b, self.g, self.r, self.a]
    }

    /// Creates a color from an RGBA byte array `[r, g, b, a]`.
    #[inline]
    pub const fn from_rgba_bytes(bytes: [u8; 4]) -> Self {
        Self {
            r: bytes[0],
            g: bytes[1],
            b: bytes[2],
            a: bytes[3],
        }
    }

    /// Returns a copy of this color with the specified alpha channel.
    #[inline]
    pub const fn with_alpha(self, a: u8) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a,
        }
    }

    /// Returns a copy of this color with alpha scaled by a normalized float `0.0..=1.0`.
    #[inline]
    pub fn with_alpha_f32(self, alpha: f32) -> Self {
        let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        self.with_alpha(a)
    }

    /// Returns the alpha channel normalized as a float between `0.0` and `1.0`.
    #[inline]
    pub fn alpha_f32(self) -> f32 {
        self.a as f32 / 255.0
    }

    /// Returns `true` if the color is fully opaque (`a == 255`).
    #[inline]
    pub fn is_opaque(&self) -> bool {
        self.a == 255
    }

    /// Returns `true` if the color is fully transparent (`a == 0`).
    #[inline]
    pub fn is_transparent(&self) -> bool {
        self.a == 0
    }

    /// Linearly interpolates between `self` and `other` by parameter `t`.
    pub fn lerp(self, other: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let r = (self.r as f32 + (other.r as f32 - self.r as f32) * t).round() as u8;
        let g = (self.g as f32 + (other.g as f32 - self.g as f32) * t).round() as u8;
        let b = (self.b as f32 + (other.b as f32 - self.b as f32) * t).round() as u8;
        let a = (self.a as f32 + (other.a as f32 - self.a as f32) * t).round() as u8;
        Color::rgba(r, g, b, a)
    }

    /// Computes the relative luminance per WCAG 2.1 (normalized `0.0..=1.0`).
    pub fn luminance(&self) -> f32 {
        fn channel_lum(val: u8) -> f32 {
            let v = val as f32 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel_lum(self.r) + 0.7152 * channel_lum(self.g) + 0.0722 * channel_lum(self.b)
    }

    /// Computes the WCAG 2.1 contrast ratio between two colors (ranging from `1.0` to `21.0`).
    pub fn contrast_ratio(&self, other: Color) -> f32 {
        let l1 = self.luminance();
        let l2 = other.luminance();
        let (bright, dark) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
        (bright + 0.05) / (dark + 0.05)
    }

    /// Parses a CSS-style hex color string like `"#FF9933"` or `"#F93"`.
    ///
    /// Safely handles UTF-8 input, leading/trailing whitespace, and returns `None` for invalid strings.
    /// Supports 3-char (`#RGB`), 4-char (`#RGBA`), 6-char (`#RRGGBB`), and 8-char (`#RRGGBBAA`) forms.
    pub fn from_hex_str(s: &str) -> Option<Self> {
        let s = s.trim();
        let s = s.strip_prefix('#').unwrap_or(s);
        // Defend against multi-byte UTF-8 panics during byte slicing
        if !s.is_ascii() {
            return None;
        }

        match s.len() {
            3 => {
                let r = u8::from_str_radix(&s[0..1], 16).ok()?;
                let g = u8::from_str_radix(&s[1..2], 16).ok()?;
                let b = u8::from_str_radix(&s[2..3], 16).ok()?;
                Some(Self::rgb(r << 4 | r, g << 4 | g, b << 4 | b))
            }
            4 => {
                let r = u8::from_str_radix(&s[0..1], 16).ok()?;
                let g = u8::from_str_radix(&s[1..2], 16).ok()?;
                let b = u8::from_str_radix(&s[2..3], 16).ok()?;
                let a = u8::from_str_radix(&s[3..4], 16).ok()?;
                Some(Self::rgba(r << 4 | r, g << 4 | g, b << 4 | b, a << 4 | a))
            }
            6 => {
                let hex = u32::from_str_radix(s, 16).ok()?;
                Some(Self::from_hex(hex))
            }
            8 => {
                let hex = u32::from_str_radix(s, 16).ok()?;
                Some(Self::rgba(
                    ((hex >> 24) & 0xFF) as u8,
                    ((hex >> 16) & 0xFF) as u8,
                    ((hex >> 8) & 0xFF) as u8,
                    (hex & 0xFF) as u8,
                ))
            }
            _ => None,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

impl From<u32> for Color {
    /// Converts from a `0xRRGGBB` value (alpha defaults to 255).
    fn from(hex: u32) -> Self {
        Self::from_hex(hex)
    }
}

impl From<Color> for u32 {
    /// Converts to `0xAARRGGBB` format.
    fn from(c: Color) -> u32 {
        c.to_argb_u32()
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a == 255 {
            write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            write!(
                f,
                "#{:02X}{:02X}{:02X}{:02X}",
                self.r, self.g, self.b, self.a
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_hex_and_bytes() {
        let c = Color::from_hex(0xFF9933);
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 153);
        assert_eq!(c.b, 51);
        assert_eq!(c.a, 255);
        assert_eq!(c.to_rgba_bytes(), [255, 153, 51, 255]);
        assert_eq!(c.to_bgra_bytes(), [51, 153, 255, 255]);
        assert_eq!(Color::from_rgba_bytes([255, 153, 51, 255]), c);
        assert_eq!(Color::from_argb_u32(0xFFFF9933), c);
        assert_eq!(Color::from_rgba_u32(0xFF9933FF), c);
        assert_eq!(c.to_rgba_u32(), 0xFF9933FF);
    }

    #[test]
    fn test_color_lerp_and_alpha() {
        let black = Color::BLACK;
        let white = Color::WHITE;
        let mid = black.lerp(white, 0.5);
        assert_eq!(mid, Color::rgb(128, 128, 128));

        let half = white.with_alpha(128);
        assert_eq!(half.a, 128);
        assert!(!half.is_opaque());
        assert!(!half.is_transparent());

        let transparent = white.with_alpha(0);
        assert!(transparent.is_transparent());
    }

    #[test]
    fn test_color_from_hex_str() {
        assert_eq!(
            Color::from_hex_str("#FF9933"),
            Some(Color::rgb(255, 153, 51))
        );
        assert_eq!(Color::from_hex_str("#FFF"), Some(Color::WHITE));
        assert_eq!(
            Color::from_hex_str("#FF993380"),
            Some(Color::rgba(255, 153, 51, 128))
        );
        assert_eq!(
            Color::from_hex_str("#F938"),
            Some(Color::rgba(255, 153, 51, 136))
        );
        assert_eq!(
            Color::from_hex_str("  FF9933  "),
            Some(Color::rgb(255, 153, 51))
        );
        assert_eq!(Color::from_hex_str("#ZZZZZZ"), None);
        assert_eq!(Color::from_hex_str("#12"), None);
        // Multi-byte UTF-8 test: must NOT panic
        assert_eq!(Color::from_hex_str("€"), None);
        assert_eq!(Color::from_hex_str("#€€"), None);
        assert_eq!(Color::from_hex_str("🦀"), None);
    }

    #[test]
    fn test_color_luminance_and_contrast() {
        assert_eq!(Color::WHITE.luminance(), 1.0);
        assert_eq!(Color::BLACK.luminance(), 0.0);
        let ratio = Color::BLACK.contrast_ratio(Color::WHITE);
        assert!((ratio - 21.0).abs() < 0.01);
    }
}
