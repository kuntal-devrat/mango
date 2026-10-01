//! Float positioning and clearance context.
//!
//! Implements CSS 2.1 §9.5: Floating elements and clearance.

use mango_core::Rect;
use mango_css::values::Clear;

/// Tracks active floating boxes within a formatting context to calculate clearance
/// and available horizontal space for in-flow content.
#[derive(Debug, Clone, Default)]
pub struct FloatContext {
    pub left_floats: Vec<Rect>,
    pub right_floats: Vec<Rect>,
}

impl FloatContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a left float margin box.
    pub fn add_left_float(&mut self, rect: Rect) {
        self.left_floats.push(rect);
    }

    /// Adds a right float margin box.
    pub fn add_right_float(&mut self, rect: Rect) {
        self.right_floats.push(rect);
    }

    /// Computes the clearance adjustment: shifts Y down past the bottom of relevant floats.
    pub fn apply_clearance(&self, current_y: f32, clear: Clear) -> f32 {
        match clear {
            Clear::None => current_y,
            Clear::Left => {
                let max_bottom = self
                    .left_floats
                    .iter()
                    .map(|f| f.bottom())
                    .fold(current_y, f32::max);
                max_bottom.max(current_y)
            }
            Clear::Right => {
                let max_bottom = self
                    .right_floats
                    .iter()
                    .map(|f| f.bottom())
                    .fold(current_y, f32::max);
                max_bottom.max(current_y)
            }
            Clear::Both => {
                let left_bottom = self
                    .left_floats
                    .iter()
                    .map(|f| f.bottom())
                    .fold(current_y, f32::max);
                let right_bottom = self
                    .right_floats
                    .iter()
                    .map(|f| f.bottom())
                    .fold(current_y, f32::max);
                left_bottom.max(right_bottom).max(current_y)
            }
        }
    }

    /// Determines available horizontal range `(start_x, end_x)` at vertical band `[y, y + height]`.
    pub fn available_span(
        &self,
        y: f32,
        height: f32,
        container_x: f32,
        container_width: f32,
    ) -> (f32, f32) {
        let bottom = y + height;
        let mut min_x = container_x;
        let mut max_x = container_x + container_width;

        for float in &self.left_floats {
            // Check if vertical ranges overlap
            if float.y() < bottom && float.bottom() > y {
                min_x = min_x.max(float.right());
            }
        }

        for float in &self.right_floats {
            if float.y() < bottom && float.bottom() > y {
                max_x = max_x.min(float.x());
            }
        }

        if min_x > max_x {
            (min_x, min_x)
        } else {
            (min_x, max_x)
        }
    }

    /// Clears all active floats.
    pub fn clear(&mut self) {
        self.left_floats.clear();
        self.right_floats.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_float_context_clearance() {
        let mut ctx = FloatContext::new();
        ctx.add_left_float(Rect::new(0.0, 10.0, 50.0, 100.0)); // bottom = 110
        ctx.add_right_float(Rect::new(200.0, 20.0, 50.0, 150.0)); // bottom = 170

        assert_eq!(ctx.apply_clearance(50.0, Clear::None), 50.0);
        assert_eq!(ctx.apply_clearance(50.0, Clear::Left), 110.0);
        assert_eq!(ctx.apply_clearance(50.0, Clear::Right), 170.0);
        assert_eq!(ctx.apply_clearance(50.0, Clear::Both), 170.0);
    }

    #[test]
    fn test_float_available_span() {
        let mut ctx = FloatContext::new();
        ctx.add_left_float(Rect::new(10.0, 0.0, 40.0, 50.0)); // span 10..50
        ctx.add_right_float(Rect::new(160.0, 0.0, 30.0, 50.0)); // span 160..190

        let (left, right) = ctx.available_span(10.0, 20.0, 0.0, 200.0);
        assert_eq!(left, 50.0);
        assert_eq!(right, 160.0);

        // Outside vertical float bounds
        let (left2, right2) = ctx.available_span(60.0, 20.0, 0.0, 200.0);
        assert_eq!(left2, 0.0);
        assert_eq!(right2, 200.0);
    }
}
