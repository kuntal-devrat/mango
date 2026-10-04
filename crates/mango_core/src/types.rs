//! Shared types used across the Mango browser engine.
//!
//! These types are the common currency between crates — geometry, colors,
//! and identifiers that every layer needs to speak.
//!
//! All types are `Send + Sync`, `Copy`, and `Clone`, suitable for use in
//! parallel layout, style, and render passes.

use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

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

    /// Returns `true` if both coordinates are finite numbers (not NaN or infinite).
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Euclidean distance to another point.
    #[inline]
    pub fn distance(&self, other: Point) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    /// Squared Euclidean distance to another point (avoids square root).
    #[inline]
    pub fn distance_squared(&self, other: Point) -> f32 {
        (self.x - other.x).powi(2) + (self.y - other.y).powi(2)
    }

    /// Linearly interpolates between `self` and `other` by parameter `t`.
    #[inline]
    pub fn lerp(&self, other: Point, t: f32) -> Point {
        Point::new(
            self.x + (other.x - self.x) * t,
            self.y + (other.y - self.y) * t,
        )
    }
}

impl Add<Point> for Point {
    type Output = Point;
    #[inline]
    fn add(self, rhs: Point) -> Point {
        Point::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign<Point> for Point {
    #[inline]
    fn add_assign(&mut self, rhs: Point) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub<Point> for Point {
    type Output = Point;
    #[inline]
    fn sub(self, rhs: Point) -> Point {
        Point::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign<Point> for Point {
    #[inline]
    fn sub_assign(&mut self, rhs: Point) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Add<Size> for Point {
    type Output = Point;
    #[inline]
    fn add(self, rhs: Size) -> Point {
        Point::new(self.x + rhs.width, self.y + rhs.height)
    }
}

impl AddAssign<Size> for Point {
    #[inline]
    fn add_assign(&mut self, rhs: Size) {
        self.x += rhs.width;
        self.y += rhs.height;
    }
}

impl Sub<Size> for Point {
    type Output = Point;
    #[inline]
    fn sub(self, rhs: Size) -> Point {
        Point::new(self.x - rhs.width, self.y - rhs.height)
    }
}

impl SubAssign<Size> for Point {
    #[inline]
    fn sub_assign(&mut self, rhs: Size) {
        self.x -= rhs.width;
        self.y -= rhs.height;
    }
}

impl Mul<f32> for Point {
    type Output = Point;
    #[inline]
    fn mul(self, rhs: f32) -> Point {
        Point::new(self.x * rhs, self.y * rhs)
    }
}

impl Mul<Point> for f32 {
    type Output = Point;
    #[inline]
    fn mul(self, rhs: Point) -> Point {
        Point::new(self * rhs.x, self * rhs.y)
    }
}

impl MulAssign<f32> for Point {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl Div<f32> for Point {
    type Output = Point;
    #[inline]
    fn div(self, rhs: f32) -> Point {
        Point::new(self.x / rhs, self.y / rhs)
    }
}

impl DivAssign<f32> for Point {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
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

    /// Returns `true` if both dimensions are finite numbers (not NaN or infinite).
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.width.is_finite() && self.height.is_finite()
    }

    /// Returns `true` if the size has zero, negative, or NaN width or height.
    #[inline]
    pub fn is_empty(&self) -> bool {
        !(self.width > 0.0 && self.height > 0.0)
    }

    /// Aspect ratio (`width / height`). Returns `0.0` if height is zero or non-positive.
    #[inline]
    pub fn aspect_ratio(&self) -> f32 {
        if self.height > 0.0 {
            self.width / self.height
        } else {
            0.0
        }
    }

    /// Clamps width and height between `min` and `max` sizes.
    #[inline]
    pub fn clamp(&self, min: Size, max: Size) -> Size {
        Size::new(
            self.width.max(min.width).min(max.width),
            self.height.max(min.height).min(max.height),
        )
    }

    /// Converts this size into a `Rect` positioned at the origin.
    #[inline]
    pub fn to_rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width, self.height)
    }
}

impl Add<Size> for Size {
    type Output = Size;
    #[inline]
    fn add(self, rhs: Size) -> Size {
        Size::new(self.width + rhs.width, self.height + rhs.height)
    }
}

impl AddAssign<Size> for Size {
    #[inline]
    fn add_assign(&mut self, rhs: Size) {
        self.width += rhs.width;
        self.height += rhs.height;
    }
}

impl Sub<Size> for Size {
    type Output = Size;
    #[inline]
    fn sub(self, rhs: Size) -> Size {
        Size::new(self.width - rhs.width, self.height - rhs.height)
    }
}

impl SubAssign<Size> for Size {
    #[inline]
    fn sub_assign(&mut self, rhs: Size) {
        self.width -= rhs.width;
        self.height -= rhs.height;
    }
}

impl Mul<f32> for Size {
    type Output = Size;
    #[inline]
    fn mul(self, rhs: f32) -> Size {
        Size::new(self.width * rhs, self.height * rhs)
    }
}

impl Mul<Size> for f32 {
    type Output = Size;
    #[inline]
    fn mul(self, rhs: Size) -> Size {
        Size::new(self * rhs.width, self * rhs.height)
    }
}

impl MulAssign<f32> for Size {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.width *= rhs;
        self.height *= rhs;
    }
}

impl Div<f32> for Size {
    type Output = Size;
    #[inline]
    fn div(self, rhs: f32) -> Size {
        Size::new(self.width / rhs, self.height / rhs)
    }
}

impl DivAssign<f32> for Size {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.width /= rhs;
        self.height /= rhs;
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

    /// Creates a new rectangle from origin and size.
    pub const fn from_origin_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    /// Creates a rectangle from left, top, right, and bottom coordinates.
    pub fn from_ltrb(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
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
    /// Left edge x-coordinate. Alias for [`x`](Self::x).
    #[inline]
    pub fn left(&self) -> f32 {
        self.origin.x
    }
    /// Top edge y-coordinate. Alias for [`y`](Self::y).
    #[inline]
    pub fn top(&self) -> f32 {
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
    /// Center point of the rectangle.
    #[inline]
    pub fn center(&self) -> Point {
        Point::new(
            self.origin.x + self.size.width * 0.5,
            self.origin.y + self.size.height * 0.5,
        )
    }

    /// Returns `true` if all coordinates and dimensions are finite numbers (not NaN or infinite).
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.origin.is_finite() && self.size.is_finite()
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

    /// Returns `true` if this rectangle completely encloses `other`.
    #[inline]
    pub fn contains_rect(&self, other: &Rect) -> bool {
        other.x() >= self.x()
            && other.right() <= self.right()
            && other.y() >= self.y()
            && other.bottom() <= self.bottom()
    }

    /// Returns `true` if this rectangle overlaps with `other`.
    #[inline]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x() < other.right()
            && self.right() > other.x()
            && self.y() < other.bottom()
            && self.bottom() > other.y()
    }

    /// Returns `true` if the rectangle has zero or negative width or height.
    #[inline]
    pub fn is_empty(&self) -> bool {
        !(self.size.width > 0.0 && self.size.height > 0.0)
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

    /// Expands the rectangle outwards by the specified edge sizes.
    #[inline]
    pub fn outset(&self, edges: EdgeSizes) -> Rect {
        Rect::new(
            self.origin.x - edges.left,
            self.origin.y - edges.top,
            (self.size.width + edges.horizontal()).max(0.0),
            (self.size.height + edges.vertical()).max(0.0),
        )
    }

    /// Shrinks the rectangle inwards by the specified edge sizes.
    #[inline]
    pub fn inset(&self, edges: EdgeSizes) -> Rect {
        Rect::new(
            self.origin.x + edges.left,
            self.origin.y + edges.top,
            (self.size.width - edges.horizontal()).max(0.0),
            (self.size.height - edges.vertical()).max(0.0),
        )
    }

    /// Translates the rectangle origin by the given offset.
    #[inline]
    pub fn translate(&self, offset: Point) -> Rect {
        Rect::new(
            self.origin.x + offset.x,
            self.origin.y + offset.y,
            self.size.width,
            self.size.height,
        )
    }

    /// Snaps rectangle origin and dimensions to whole integer device pixels.
    #[inline]
    pub fn pixel_snapped(&self) -> Rect {
        let x = self.origin.x.floor();
        let y = self.origin.y.floor();
        let right = (self.origin.x + self.size.width).ceil();
        let bottom = (self.origin.y + self.size.height).ceil();
        Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
    }

    /// Rounds origin and size components to the nearest integer.
    #[inline]
    pub fn round(&self) -> Rect {
        Rect::new(
            self.origin.x.round(),
            self.origin.y.round(),
            self.size.width.round(),
            self.size.height.round(),
        )
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

    /// Returns the intersection of two rectangles, or `Rect::ZERO` if disjoint.
    #[inline]
    pub fn intersection_or_empty(&self, other: &Rect) -> Rect {
        self.intersect(other).unwrap_or(Rect::ZERO)
    }

    /// Returns the smallest rectangle enclosing both `self` and `other`.
    #[inline]
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x().min(other.x());
        let y = self.y().min(other.y());
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
    }

    /// Union that skips empty rectangles.
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

impl Add<Point> for Rect {
    type Output = Rect;
    #[inline]
    fn add(self, rhs: Point) -> Rect {
        self.translate(rhs)
    }
}

impl Sub<Point> for Rect {
    type Output = Rect;
    #[inline]
    fn sub(self, rhs: Point) -> Rect {
        self.translate(Point::new(-rhs.x, -rhs.y))
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

/// Corner radii for rectangles (e.g. CSS `border-radius`).
///
/// Follows clockwise order starting from top-left: `[top_left, top_right, bottom_right, bottom_left]`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CornerRadii {
    /// Top-left corner radius.
    pub top_left: f32,
    /// Top-right corner radius.
    pub top_right: f32,
    /// Bottom-right corner radius.
    pub bottom_right: f32,
    /// Bottom-left corner radius.
    pub bottom_left: f32,
}

impl CornerRadii {
    /// Zero corner radii (sharp corners).
    pub const ZERO: CornerRadii = CornerRadii {
        top_left: 0.0,
        top_right: 0.0,
        bottom_right: 0.0,
        bottom_left: 0.0,
    };

    /// Creates uniform corner radii where all four corners have the same radius.
    pub const fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    /// Creates corner radii with individual values for each corner.
    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    /// Returns `true` if all radii are zero.
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.top_left == 0.0
            && self.top_right == 0.0
            && self.bottom_right == 0.0
            && self.bottom_left == 0.0
    }

    /// Returns `true` if all four corners have the identical radius.
    #[inline]
    pub fn is_uniform(&self) -> bool {
        self.top_left == self.top_right
            && self.top_right == self.bottom_right
            && self.bottom_right == self.bottom_left
    }

    /// Returns `true` if all radii are finite numbers.
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.top_left.is_finite()
            && self.top_right.is_finite()
            && self.bottom_right.is_finite()
            && self.bottom_left.is_finite()
    }

    /// Converts to an array `[top_left, top_right, bottom_right, bottom_left]`.
    #[inline]
    pub const fn to_array(&self) -> [f32; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }
}

impl From<f32> for CornerRadii {
    fn from(r: f32) -> Self {
        Self::all(r)
    }
}

impl From<[f32; 4]> for CornerRadii {
    fn from(arr: [f32; 4]) -> Self {
        Self::new(arr[0], arr[1], arr[2], arr[3])
    }
}

impl From<CornerRadii> for [f32; 4] {
    fn from(r: CornerRadii) -> Self {
        r.to_array()
    }
}

impl Mul<f32> for CornerRadii {
    type Output = CornerRadii;
    #[inline]
    fn mul(self, rhs: f32) -> CornerRadii {
        CornerRadii::new(
            self.top_left * rhs,
            self.top_right * rhs,
            self.bottom_right * rhs,
            self.bottom_left * rhs,
        )
    }
}

impl Div<f32> for CornerRadii {
    type Output = CornerRadii;
    #[inline]
    fn div(self, rhs: f32) -> CornerRadii {
        CornerRadii::new(
            self.top_left / rhs,
            self.top_right / rhs,
            self.bottom_right / rhs,
            self.bottom_left / rhs,
        )
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

    /// Creates edge sizes with vertical and horizontal dimensions (CSS 2-value shorthand).
    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
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

    /// Returns `true` if all edge sizes are exactly zero.
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0 && self.left == 0.0
    }

    /// Returns `true` if all four edge sizes are identical.
    #[inline]
    pub fn is_uniform(&self) -> bool {
        self.top == self.right && self.right == self.bottom && self.bottom == self.left
    }

    /// Returns `true` if all edge sizes are finite numbers.
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.top.is_finite()
            && self.right.is_finite()
            && self.bottom.is_finite()
            && self.left.is_finite()
    }

    /// Returns an `EdgeSizes` with each side set to the maximum of `self` and `other` (e.g. for margin collapsing).
    pub fn max(&self, other: &Self) -> Self {
        Self::new(
            self.top.max(other.top),
            self.right.max(other.right),
            self.bottom.max(other.bottom),
            self.left.max(other.left),
        )
    }

    /// Clamps all sides to be at least `min`.
    pub fn clamp_min(&self, min: f32) -> Self {
        Self::new(
            self.top.max(min),
            self.right.max(min),
            self.bottom.max(min),
            self.left.max(min),
        )
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

impl AddAssign for EdgeSizes {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.top += rhs.top;
        self.right += rhs.right;
        self.bottom += rhs.bottom;
        self.left += rhs.left;
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

impl SubAssign for EdgeSizes {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.top -= rhs.top;
        self.right -= rhs.right;
        self.bottom -= rhs.bottom;
        self.left -= rhs.left;
    }
}

impl Mul<f32> for EdgeSizes {
    type Output = EdgeSizes;
    #[inline]
    fn mul(self, rhs: f32) -> EdgeSizes {
        EdgeSizes::new(
            self.top * rhs,
            self.right * rhs,
            self.bottom * rhs,
            self.left * rhs,
        )
    }
}

impl Mul<EdgeSizes> for f32 {
    type Output = EdgeSizes;
    #[inline]
    fn mul(self, rhs: EdgeSizes) -> EdgeSizes {
        EdgeSizes::new(
            self * rhs.top,
            self * rhs.right,
            self * rhs.bottom,
            self * rhs.left,
        )
    }
}

impl Div<f32> for EdgeSizes {
    type Output = EdgeSizes;
    #[inline]
    fn div(self, rhs: f32) -> EdgeSizes {
        EdgeSizes::new(
            self.top / rhs,
            self.right / rhs,
            self.bottom / rhs,
            self.left / rhs,
        )
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
        assert_send_sync::<CornerRadii>();
        assert_send_sync::<EdgeSizes>();
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_arithmetic_and_methods() {
        let p1 = Point::new(10.0, 20.0);
        let p2 = Point::new(5.0, 8.0);
        let s = Size::new(2.0, 3.0);

        assert_eq!(p1 + p2, Point::new(15.0, 28.0));
        assert_eq!(p1 - p2, Point::new(5.0, 12.0));
        assert_eq!(p1 + s, Point::new(12.0, 23.0));
        assert_eq!(p1 - s, Point::new(8.0, 17.0));
        assert_eq!(p1 * 2.0, Point::new(20.0, 40.0));
        assert_eq!(p1 / 2.0, Point::new(5.0, 10.0));
        assert_eq!(p1.distance(Point::new(13.0, 24.0)), 5.0);
        assert_eq!(p1.distance_squared(Point::new(13.0, 24.0)), 25.0);
        assert_eq!(
            Point::ZERO.lerp(Point::new(10.0, 20.0), 0.5),
            Point::new(5.0, 10.0)
        );
        assert!(p1.is_finite());
    }

    #[test]
    fn test_size_arithmetic_and_methods() {
        let s1 = Size::new(100.0, 50.0);
        let s2 = Size::new(20.0, 10.0);

        assert_eq!(s1 + s2, Size::new(120.0, 60.0));
        assert_eq!(s1 - s2, Size::new(80.0, 40.0));
        assert_eq!(s1 * 1.5, Size::new(150.0, 75.0));
        assert_eq!(s1 / 2.0, Size::new(50.0, 25.0));
        assert_eq!(s1.aspect_ratio(), 2.0);
        assert!(!s1.is_empty());
        assert!(Size::ZERO.is_empty());
        assert!(Size::new(-5.0, 10.0).is_empty());
        assert_eq!(
            s1.clamp(Size::new(10.0, 10.0), Size::new(80.0, 40.0)),
            Size::new(80.0, 40.0)
        );
    }

    #[test]
    fn test_rect_contains_and_overlaps() {
        let rect = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(rect.contains(Point::new(50.0, 30.0)));
        assert!(!rect.contains(Point::new(5.0, 5.0)));
        assert!(!rect.contains(Point::new(200.0, 200.0)));

        let inner = Rect::new(20.0, 20.0, 10.0, 10.0);
        assert!(rect.contains_rect(&inner));
        assert!(!inner.contains_rect(&rect));

        let overlapping = Rect::new(50.0, 30.0, 100.0, 100.0);
        assert!(rect.intersects(&overlapping));
        assert_eq!(
            rect.intersection_or_empty(&overlapping),
            Rect::new(50.0, 30.0, 60.0, 30.0)
        );

        let disjoint = Rect::new(200.0, 200.0, 10.0, 10.0);
        assert!(!rect.intersects(&disjoint));
        assert_eq!(rect.intersection_or_empty(&disjoint), Rect::ZERO);
    }

    #[test]
    fn test_rect_inset_outset_and_translation() {
        let r = Rect::new(20.0, 30.0, 100.0, 60.0);
        let edges = EdgeSizes::new(5.0, 10.0, 15.0, 20.0);

        let outset = r.outset(edges);
        assert_eq!(outset, Rect::new(0.0, 25.0, 130.0, 80.0));

        let inset = r.inset(edges);
        assert_eq!(inset, Rect::new(40.0, 35.0, 70.0, 40.0));

        let translated = r + Point::new(10.0, -10.0);
        assert_eq!(translated, Rect::new(30.0, 20.0, 100.0, 60.0));

        let snapped = Rect::new(10.2, 20.7, 100.1, 50.4).pixel_snapped();
        assert_eq!(snapped, Rect::new(10.0, 20.0, 101.0, 52.0));
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
    fn test_corner_radii() {
        let uniform = CornerRadii::all(5.0);
        assert!(uniform.is_uniform());
        assert_eq!(uniform.to_array(), [5.0, 5.0, 5.0, 5.0]);

        let individual = CornerRadii::new(1.0, 2.0, 3.0, 4.0);
        assert!(!individual.is_uniform());
        assert!(!individual.is_zero());
        assert_eq!(individual * 2.0, CornerRadii::new(2.0, 4.0, 6.0, 8.0));
    }

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
        // 4-char shorthand with alpha
        assert_eq!(
            Color::from_hex_str("#F938"),
            Some(Color::rgba(255, 153, 51, 136))
        );
        // Without hash prefix and with whitespace
        assert_eq!(
            Color::from_hex_str("  FF9933  "),
            Some(Color::rgb(255, 153, 51))
        );
        // Invalid
        assert_eq!(Color::from_hex_str("#ZZZZZZ"), None);
        assert_eq!(Color::from_hex_str("#12"), None);
        // Multi-byte UTF-8 test: must NOT panic!
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

    #[test]
    fn test_edge_sizes() {
        let edges = EdgeSizes::all(10.0);
        assert_eq!(edges.horizontal(), 20.0);
        assert_eq!(edges.vertical(), 20.0);
        assert!(edges.is_uniform());

        let sym = EdgeSizes::symmetric(5.0, 15.0);
        assert_eq!(sym.top, 5.0);
        assert_eq!(sym.bottom, 5.0);
        assert_eq!(sym.left, 15.0);
        assert_eq!(sym.right, 15.0);

        let scaled = edges * 2.0;
        assert_eq!(scaled, EdgeSizes::all(20.0));
    }

    #[test]
    fn test_union_non_empty() {
        let r = Rect::new(10.0, 10.0, 20.0, 20.0);
        assert_eq!(Rect::ZERO.union_non_empty(&r), r);
        assert_eq!(r.union_non_empty(&Rect::ZERO), r);
    }

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Point>();
        assert_send_sync::<Size>();
        assert_send_sync::<Rect>();
        assert_send_sync::<Color>();
        assert_send_sync::<CornerRadii>();
        assert_send_sync::<EdgeSizes>();
    }
}
