//! Edge sizes for CSS box model (margin, padding, border).

use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

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

    /// Creates edge sizes with individual values for each side.
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Creates uniform edge sizes where all four sides have the same value.
    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Creates symmetric edge sizes: `vertical` for top/bottom, `horizontal` for left/right.
    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Sum of the left and right edge sizes.
    #[inline]
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Sum of the top and bottom edge sizes.
    #[inline]
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }

    /// Returns `true` if all four edge sizes are equal.
    #[inline]
    pub fn is_uniform(&self) -> bool {
        self.top == self.right && self.right == self.bottom && self.bottom == self.left
    }

    /// Returns `true` if all four edge sizes are zero.
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0 && self.left == 0.0
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
