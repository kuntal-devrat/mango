//! 2D Point type with f32 coordinates and vector arithmetic.

use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

use super::size::Size;

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
}
