//! Shared types used across the Mango browser engine.
//!
//! These types are the common currency between crates — geometry, colors,
//! and identifiers that every layer needs to speak.
//!
//! All types are `Send + Sync`, `Copy`, and `Clone`, suitable for use in
//! parallel layout, style, and render passes.

use std::fmt;
use std::ops::{Add, Sub};

/// A 2D point with `f32` coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

impl Point {
    /// The origin point `(0, 0)`.
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    /// Creates a new point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

/// A 2D size with `f32` dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    /// Width dimension.
    pub width: f32,
    /// Height dimension.
    pub height: f32,
}

impl Size {
    /// A zero-area size.
    pub const ZERO: Size = Size {
        width: 0.0,
        height: 0.0,
    };

    /// Creates a new size.
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}×{}", self.width, self.height)
    }
}

/// A rectangle defined by its origin point and size.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Top-left corner of the rectangle.
    pub origin: Point,
    /// Width and height.
    pub size: Size,
}

impl Rect {
    /// A zero-area rectangle at the origin.
    pub const ZERO: Rect = Rect {
        origin: Point::ZERO,
        size: Size::ZERO,
    };

    /// Creates a new rectangle from position and dimensions.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    /// Left edge x-coordinate.
    #[inline]
    pub fn x(&self) -> f32 {
        self.origin.x
    }
    /// Top edge y-coordinate.
    #[inline]
    pub fn y(&self) -> f32 {
        self.origin.y
    }
    /// Width of the rectangle.
    #[inline]
    pub fn width(&self) -> f32 {
        self.size.width
    }
    /// Height of the rectangle.
    #[inline]
    pub fn height(&self) -> f32 {
        self.size.height
    }
    /// Right edge x-coordinate (`x + width`).
    #[inline]
    pub fn right(&self) -> f32 {
        self.origin.x + self.size.width
    }
    /// Bottom edge y-coordinate (`y + height`).
    #[inline]
    pub fn bottom(&self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Returns `true` if this rectangle contains the given point using half-open bounds `[left, right)` and `[top, bottom)`.
    #[inline]
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x()
            && point.x < self.right()
            && point.y >= self.y()
            && point.y < self.bottom()
    }

    /// Returns `true` if this rectangle contains the given point including the right and bottom edges.
    #[inline]
    pub fn contains_inclusive(&self, point: Point) -> bool {
        point.x >= self.x()
            && point.x <= self.right()
            && point.y >= self.y()
            && point.y <= self.bottom()
    }

    /// Returns `true` if the rectangle has zero or negative width or height.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.size.width <= 0.0 || self.size.height <= 0.0
    }

    /// Normalizes a rectangle with negative dimensions by moving its origin.
    pub fn normalize(&self) -> Rect {
        let mut x = self.origin.x;
        let mut y = self.origin.y;
        let mut w = self.size.width;
        let mut h = self.size.height;
        if w < 0.0 {
            x += w;
            w = -w;
        }
        if h < 0.0 {
            y += h;
            h = -h;
        }
        Rect::new(x, y, w, h)
    }

    /// Returns the intersection of two rectangles, or `None` if they don't overlap.
    #[inline]
    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let x = self.x().max(other.x());
        let y = self.y().max(other.y());
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        if right > x && bottom > y {
            Some(Rect::new(x, y, right - x, bottom - y))
        } else {
            None
        }
    }

    /// Returns the smallest rectangle enclosing both `self` and `other`.
    ///
    /// **Note:** If either rectangle is used as a zero-initialized accumulator
    /// (e.g. `Rect::ZERO`), the origin `(0, 0)` will participate in the union.
    /// Use [`union_non_empty`](Self::union_non_empty) if one operand may be a
    /// sentinel.
    #[inline]
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x().min(other.x());
        let y = self.y().min(other.y());
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
    }

    /// Union that skips empty rectangles.
    ///
    /// If `self` is empty, returns `*other`. If `other` is empty, returns
    /// `*self`. Otherwise delegates to [`union`](Self::union). This is useful
    /// when accumulating a bounding box from an initial `Rect::ZERO` sentinel.
    #[inline]
    pub fn union_non_empty(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        self.union(other)
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Rect({}, {} {}×{})",
            self.origin.x, self.origin.y, self.size.width, self.size.height
        )
    }
}

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

    /// Creates a color from a 24-bit hex value (e.g., `0xFF9933`).
    pub const fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
            a: 255,
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

    /// Parses a CSS-style hex color string like `"#FF9933"` or `"#F93"`.
    ///
    /// Returns `None` for invalid input. Supports 3-char (`#RGB`),
    /// 4-char (`#RGBA`), 6-char (`#RRGGBB`), and 8-char (`#RRGGBBAA`) forms.
    pub fn from_hex_str(s: &str) -> Option<Self> {
        let s = s.strip_prefix('#').unwrap_or(s);
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

/// Edge sizes for the box model (margin, padding, border).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EdgeSizes {
    /// Top edge size.
    pub top: f32,
    /// Right edge size.
    pub right: f32,
    /// Bottom edge size.
    pub bottom: f32,
    /// Left edge size.
    pub left: f32,
}

impl EdgeSizes {
    /// All edges set to zero.
    pub const ZERO: EdgeSizes = EdgeSizes {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// Creates edge sizes with all four sides set to the same value.
    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Creates edge sizes with explicit values for each side.
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Total horizontal space (left + right).
    #[inline]
    pub const fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Total vertical space (top + bottom).
    #[inline]
    pub const fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

impl Add for EdgeSizes {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            top: self.top + rhs.top,
            right: self.right + rhs.right,
            bottom: self.bottom + rhs.bottom,
            left: self.left + rhs.left,
        }
    }
}

impl Sub for EdgeSizes {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            top: self.top - rhs.top,
            right: self.right - rhs.right,
            bottom: self.bottom - rhs.bottom,
            left: self.left - rhs.left,
        }
    }
}

impl fmt::Display for EdgeSizes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EdgeSizes(top:{} right:{} bottom:{} left:{})",
            self.top, self.right, self.bottom, self.left
        )
    }
}

// Compile-time assertions: all types are Send+Sync.
const _: () = {
    #[allow(dead_code)]
    fn assert_send_sync<T: Send + Sync>() {}
    #[allow(dead_code)]
    fn assertions() {
        assert_send_sync::<Point>();
        assert_send_sync::<Size>();
        assert_send_sync::<Rect>();
        assert_send_sync::<Color>();
        assert_send_sync::<EdgeSizes>();
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_contains() {
        let rect = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(rect.contains(Point::new(50.0, 30.0)));
        assert!(!rect.contains(Point::new(5.0, 5.0)));
        assert!(!rect.contains(Point::new(200.0, 200.0)));
    }

    #[test]
    fn test_rect_intersect() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        let c = Rect::new(200.0, 200.0, 10.0, 10.0);

        assert!(a.intersect(&b).is_some());
        assert!(a.intersect(&c).is_none());

        let intersection = a.intersect(&b).unwrap();
        assert_eq!(intersection, Rect::new(50.0, 50.0, 50.0, 50.0));
    }

    #[test]
    fn test_color_hex() {
        let c = Color::from_hex(0xFF9933);
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 153);
        assert_eq!(c.b, 51);
        assert_eq!(c.a, 255);
    }

    #[test]
    fn test_color_to_u32() {
        let white = Color::WHITE;
        assert_eq!(white.to_rgb_u32(), 0x00FFFFFF);
    }

    #[test]
    fn test_edge_sizes() {
        let edges = EdgeSizes::all(10.0);
        assert_eq!(edges.horizontal(), 20.0);
        assert_eq!(edges.vertical(), 20.0);
    }

    #[test]
    fn test_color_from_hex_str() {
        // 6-char
        assert_eq!(
            Color::from_hex_str("#FF9933"),
            Some(Color::rgb(255, 153, 51))
        );
        // 3-char shorthand
        assert_eq!(Color::from_hex_str("#FFF"), Some(Color::WHITE));
        // 8-char with alpha
        assert_eq!(
            Color::from_hex_str("#FF993380"),
            Some(Color::rgba(255, 153, 51, 128))
        );
        // Without hash prefix
        assert_eq!(
            Color::from_hex_str("FF9933"),
            Some(Color::rgb(255, 153, 51))
        );
        // Invalid
        assert_eq!(Color::from_hex_str("#ZZZZZZ"), None);
        assert_eq!(Color::from_hex_str("#12"), None);
    }

    #[test]
    fn test_color_display() {
        assert_eq!(format!("{}", Color::WHITE), "#FFFFFF");
        assert_eq!(format!("{}", Color::TRANSPARENT), "#00000000");
    }

    #[test]
    fn test_color_from_u32() {
        let c: Color = Color::from(0xFF9933_u32);
        assert_eq!(c, Color::rgb(255, 153, 51));
        let val: u32 = Color::WHITE.into();
        assert_eq!(val, 0xFFFFFFFF);
    }

    #[test]
    fn test_edge_sizes_add_sub() {
        let a = EdgeSizes::new(1.0, 2.0, 3.0, 4.0);
        let b = EdgeSizes::new(4.0, 3.0, 2.0, 1.0);
        let sum = a + b;
        assert_eq!(sum, EdgeSizes::all(5.0));
        let diff = sum - b;
        assert_eq!(diff, a);
    }

    #[test]
    fn test_union_non_empty() {
        let r = Rect::new(10.0, 10.0, 20.0, 20.0);
        // Union with zero sentinel should return the non-empty rect
        assert_eq!(Rect::ZERO.union_non_empty(&r), r);
        assert_eq!(r.union_non_empty(&Rect::ZERO), r);
    }

    #[test]
    fn test_point_display() {
        assert_eq!(format!("{}", Point::new(1.5, 2.5)), "(1.5, 2.5)");
    }

    #[test]
    fn test_rect_display() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(format!("{r}"), "Rect(10, 20 100×50)");
    }

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Point>();
        assert_send_sync::<Size>();
        assert_send_sync::<Rect>();
        assert_send_sync::<Color>();
        assert_send_sync::<EdgeSizes>();
    }
}
