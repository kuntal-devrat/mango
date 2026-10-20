//! 2D Rect type defined by origin Point and Size.

use std::fmt;
use std::ops::{Add, Sub};

use super::edge_sizes::EdgeSizes;
use super::point::Point;
use super::size::Size;

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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_union_non_empty() {
        let r = Rect::new(10.0, 10.0, 20.0, 20.0);
        assert_eq!(Rect::ZERO.union_non_empty(&r), r);
        assert_eq!(r.union_non_empty(&Rect::ZERO), r);
    }
}
