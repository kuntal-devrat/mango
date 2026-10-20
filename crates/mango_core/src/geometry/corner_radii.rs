//! Corner radii for rectangles (e.g. CSS `border-radius`).

use std::fmt;
use std::ops::{Div, Mul};

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

    /// Returns `true` if all four corner radii are zero.
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.top_left == 0.0
            && self.top_right == 0.0
            && self.bottom_right == 0.0
            && self.bottom_left == 0.0
    }

    /// Returns `true` if all four corner radii are equal.
    #[inline]
    pub fn is_uniform(&self) -> bool {
        self.top_left == self.top_right
            && self.top_right == self.bottom_right
            && self.bottom_right == self.bottom_left
    }

    /// Returns `true` if all corner radii are finite numbers (not NaN or infinite).
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.top_left.is_finite()
            && self.top_right.is_finite()
            && self.bottom_right.is_finite()
            && self.bottom_left.is_finite()
    }

    /// Returns an array of the four corner radii: `[top_left, top_right, bottom_right, bottom_left]`.
    #[inline]
    pub fn to_array(&self) -> [f32; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }

    /// Maximum radius among all four corners.
    #[inline]
    pub fn max_radius(&self) -> f32 {
        self.top_left
            .max(self.top_right)
            .max(self.bottom_right)
            .max(self.bottom_left)
    }

    /// Scales all corner radii proportionally so that adjacent radii fit within the given `width` and `height`.
    pub fn constrain(&self, width: f32, height: f32) -> Self {
        let top = self.top_left + self.top_right;
        let right = self.top_right + self.bottom_right;
        let bottom = self.bottom_right + self.bottom_left;
        let left = self.bottom_left + self.top_left;

        let mut factor = 1.0f32;
        if top > width && width > 0.0 {
            factor = factor.min(width / top);
        }
        if bottom > width && width > 0.0 {
            factor = factor.min(width / bottom);
        }
        if right > height && height > 0.0 {
            factor = factor.min(height / right);
        }
        if left > height && height > 0.0 {
            factor = factor.min(height / left);
        }

        if factor < 1.0 { *self * factor } else { *self }
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

impl fmt::Display for CornerRadii {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CornerRadii(tl:{} tr:{} br:{} bl:{})",
            self.top_left, self.top_right, self.bottom_right, self.bottom_left
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
