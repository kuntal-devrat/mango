//! 2D Size type with f32 dimensions.

use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// A 2D size with `f32` dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    /// Width dimension.
    pub width: f32,
    /// Height dimension.
    pub height: f32,
}

impl Size {
    /// A zero size `(0, 0)`.
    pub const ZERO: Size = Size {
        width: 0.0,
        height: 0.0,
    };

    /// Creates a new size.
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Creates a square size `(side, side)`.
    pub const fn square(side: f32) -> Self {
        Self {
            width: side,
            height: side,
        }
    }

    /// Returns `true` if width <= 0 or height <= 0, or either is NaN.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0 || self.width.is_nan() || self.height.is_nan()
    }

    /// Returns `true` if both dimensions are finite numbers.
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.width.is_finite() && self.height.is_finite()
    }

    /// Returns the area `width * height`. Returns 0 if empty.
    #[inline]
    pub fn area(&self) -> f32 {
        if self.is_empty() {
            0.0
        } else {
            self.width * self.height
        }
    }

    /// Aspect ratio `width / height`. Returns 0 if height is 0.
    #[inline]
    pub fn aspect_ratio(&self) -> f32 {
        if self.height == 0.0 {
            0.0
        } else {
            self.width / self.height
        }
    }

    /// Clamps this size within the given `min` and `max` constraints.
    #[inline]
    pub fn clamp(&self, min: Size, max: Size) -> Size {
        Size::new(
            self.width.clamp(min.width, max.width),
            self.height.clamp(min.height, max.height),
        )
    }

    /// Returns a new size with each dimension being the maximum of `self` and `other`.
    #[inline]
    pub fn max(&self, other: Size) -> Size {
        Size::new(self.width.max(other.width), self.height.max(other.height))
    }

    /// Returns a new size with each dimension being the minimum of `self` and `other`.
    #[inline]
    pub fn min(&self, other: Size) -> Size {
        Size::new(self.width.min(other.width), self.height.min(other.height))
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
