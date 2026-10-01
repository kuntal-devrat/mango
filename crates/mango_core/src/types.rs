//! Shared types used across the Mango browser engine.
//!
//! These types are the common currency between crates — geometry, colors,
//! and identifiers that every layer needs to speak.

/// A 2D point with `f32` coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A 2D size with `f32` dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Size = Size {
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// A rectangle defined by its origin point and size.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const ZERO: Rect = Rect {
        origin: Point::ZERO,
        size: Size::ZERO,
    };

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    #[inline]
    pub fn x(&self) -> f32 {
        self.origin.x
    }
    #[inline]
    pub fn y(&self) -> f32 {
        self.origin.y
    }
    #[inline]
    pub fn width(&self) -> f32 {
        self.size.width
    }
    #[inline]
    pub fn height(&self) -> f32 {
        self.size.height
    }
    #[inline]
    pub fn right(&self) -> f32 {
        self.origin.x + self.size.width
    }
    #[inline]
    pub fn bottom(&self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Returns `true` if this rectangle contains the given point using half-open bounds `[left, right)` and `[top, bottom)`.
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x()
            && point.x < self.right()
            && point.y >= self.y()
            && point.y < self.bottom()
    }

    /// Returns `true` if this rectangle contains the given point including the right and bottom edges.
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
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x().min(other.x());
        let y = self.y().min(other.y());
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
    }
}

/// An RGBA color with 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const RED: Color = Color::rgb(255, 0, 0);
    pub const GREEN: Color = Color::rgb(0, 128, 0);
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);

    // Mango brand colors
    pub const MANGO_ORANGE: Color = Color::rgb(255, 161, 54);
    pub const MANGO_DARK: Color = Color::rgb(30, 30, 30);
    pub const MANGO_LIGHT: Color = Color::rgb(245, 245, 240);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

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
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

/// Edge sizes for the box model (margin, padding, border).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EdgeSizes {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl EdgeSizes {
    pub const ZERO: EdgeSizes = EdgeSizes {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

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
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Total vertical space (top + bottom).
    #[inline]
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

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
}
