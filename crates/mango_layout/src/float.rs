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

    /// Returns `true` if there are no active floats.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.left_floats.is_empty() && self.right_floats.is_empty()
    }

    /// Returns `true` if there are active floats.
    #[inline]
    pub fn has_floats(&self) -> bool {
        !self.is_empty()
    }

    /// Returns the maximum bottom margin edge of all left floats, if any.
    pub fn left_bottom(&self) -> Option<f32> {
        self.left_floats
            .iter()
            .map(|f| f.bottom())
            .fold(None, |acc, b| Some(acc.map_or(b, |p: f32| p.max(b))))
    }

    /// Returns the maximum bottom margin edge of all right floats, if any.
    pub fn right_bottom(&self) -> Option<f32> {
        self.right_floats
            .iter()
            .map(|f| f.bottom())
            .fold(None, |acc, b| Some(acc.map_or(b, |p: f32| p.max(b))))
    }

    /// Returns the maximum bottom margin edge of all active floats (CSS 2.1 §10.6.7).
    pub fn max_bottom(&self) -> Option<f32> {
        match (self.left_bottom(), self.right_bottom()) {
            (Some(l), Some(r)) => Some(l.max(r)),
            (Some(l), None) => Some(l),
            (None, Some(r)) => Some(r),
            (None, None) => None,
        }
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

    /// Computes clearance adjustment taking the element's top margin into account (CSS 2.1 §9.5.2).
    ///
    /// Returns `(cleared_y, has_clearance)` where `cleared_y` is the y coordinate where
    /// the element's margin or border should be placed, and `has_clearance` indicates whether
    /// clearance was required.
    pub fn apply_clearance_with_margin(
        &self,
        current_y: f32,
        margin_top: f32,
        clear: Clear,
    ) -> (f32, bool) {
        match clear {
            Clear::None => (current_y, false),
            Clear::Left | Clear::Right | Clear::Both => {
                let max_bottom = match clear {
                    Clear::Left => self.left_bottom(),
                    Clear::Right => self.right_bottom(),
                    Clear::Both => self.max_bottom(),
                    Clear::None => unreachable!(),
                };
                if let Some(bottom) = max_bottom {
                    let hypothetical_border_top = current_y + margin_top;
                    if hypothetical_border_top < bottom {
                        (bottom, true)
                    } else {
                        (current_y, false)
                    }
                } else {
                    (current_y, false)
                }
            }
        }
    }

    /// Determines available horizontal range `(start_x, end_x)` at vertical band `[y, y + height]`.
    ///
    /// For zero or negative height, probes an instantaneous point at `y` (CSS 2.1 §9.5).
    pub fn available_span(
        &self,
        y: f32,
        height: f32,
        container_x: f32,
        container_width: f32,
    ) -> (f32, f32) {
        if self.is_empty() {
            return (container_x, container_x + container_width);
        }

        // For zero or negative heights, probe an epsilon band at y so floats starting at y are matched.
        let bottom = if height <= 0.0 { y + 0.001 } else { y + height };
        let mut min_x = container_x;
        let mut max_x = container_x + container_width;

        for float in &self.left_floats {
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

    /// Returns the next vertical opportunity (earliest float bottom below `y`) to step down past obstruction (CSS 2.1 §9.5.1 Rules 7-9).
    pub fn next_vertical_opportunity(&self, y: f32) -> Option<f32> {
        let mut min_bottom = f32::INFINITY;
        for f in self.left_floats.iter().chain(self.right_floats.iter()) {
            if f.bottom() > y + 0.001 && f.bottom() < min_bottom {
                min_bottom = f.bottom();
            }
        }
        if min_bottom.is_finite() {
            Some(min_bottom)
        } else {
            None
        }
    }

    /// Finds the `(x, y)` position of a float's margin box following CSS 2.1 §9.5.1 Rules 1–9.
    ///
    /// Steps down past any conflicting active floats until the float fits horizontally,
    /// placing left floats as far left as possible and right floats as far right as possible.
    pub fn find_float_position(
        &self,
        is_right: bool,
        initial_y: f32,
        margin_box_w: f32,
        margin_box_h: f32,
        container_x: f32,
        container_width: f32,
    ) -> (f32, f32) {
        let mut float_y = initial_y;
        let max_steps = self.left_floats.len() + self.right_floats.len() + 1;
        let container_right = container_x + container_width;

        for _ in 0..max_steps {
            let (min_x, max_x) =
                self.available_span(float_y, margin_box_h, container_x, container_width);
            let avail_w = max_x - min_x;

            if margin_box_w <= avail_w
                || (min_x <= container_x && max_x >= container_right)
            {
                let x = if is_right {
                    max_x - margin_box_w
                } else {
                    min_x
                };
                return (x, float_y);
            }

            if let Some(next_y) = self.next_vertical_opportunity(float_y) {
                if next_y > float_y {
                    float_y = next_y;
                    continue;
                }
            }
            break;
        }

        let x = if is_right {
            container_right - margin_box_w
        } else {
            container_x
        };
        (x, float_y)
    }

    /// Returns the horizontal space consumed by left floats at vertical band `[y, y + height]`.
    pub fn left_float_offset(&self, y: f32, height: f32, container_x: f32) -> f32 {
        let (min_x, _) = self.available_span(y, height, container_x, f32::INFINITY);
        (min_x - container_x).max(0.0)
    }

    /// Returns the horizontal space consumed by right floats at vertical band `[y, y + height]`.
    pub fn right_float_offset(
        &self,
        y: f32,
        height: f32,
        container_x: f32,
        container_width: f32,
    ) -> f32 {
        let (_, max_x) = self.available_span(y, height, container_x, container_width);
        (container_x + container_width - max_x).max(0.0)
    }

    /// Returns `true` if any float intersects the vertical band `[y, y + height]`.
    pub fn has_floats_at(&self, y: f32, height: f32) -> bool {
        let bottom = if height <= 0.0 { y + 0.001 } else { y + height };
        self.left_floats
            .iter()
            .chain(self.right_floats.iter())
            .any(|f| f.y() < bottom && f.bottom() > y)
    }

    /// Computes the bounding box of all active floats, if any.
    pub fn total_bounds(&self) -> Option<Rect> {
        let mut iter = self.left_floats.iter().chain(self.right_floats.iter());
        let first = iter.next()?.clone();
        let mut min_x = first.x();
        let mut min_y = first.y();
        let mut max_x = first.right();
        let mut max_y = first.bottom();

        for f in iter {
            min_x = min_x.min(f.x());
            min_y = min_y.min(f.y());
            max_x = max_x.max(f.right());
            max_y = max_y.max(f.bottom());
        }

        Some(Rect::new(
            min_x,
            min_y,
            (max_x - min_x).max(0.0),
            (max_y - min_y).max(0.0),
        ))
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

    #[test]
    fn test_zero_height_available_span_at_float_top() {
        let mut ctx = FloatContext::new();
        ctx.add_left_float(Rect::new(0.0, 25.0, 60.0, 100.0));

        // Probing with height = 0 at the exact top edge of the float (25.0)
        let (left, _) = ctx.available_span(25.0, 0.0, 0.0, 300.0);
        assert_eq!(left, 60.0, "Zero-height probe at float top edge must detect float");
    }

    #[test]
    fn test_next_vertical_opportunity_stepping() {
        let mut ctx = FloatContext::new();
        ctx.add_left_float(Rect::new(0.0, 10.0, 50.0, 40.0)); // bottom = 50
        ctx.add_right_float(Rect::new(200.0, 20.0, 50.0, 80.0)); // bottom = 100

        assert_eq!(ctx.next_vertical_opportunity(5.0), Some(50.0));
        assert_eq!(ctx.next_vertical_opportunity(50.0), Some(100.0));
        assert_eq!(ctx.next_vertical_opportunity(100.0), None);
    }

    #[test]
    fn test_find_float_position_left_and_right() {
        let mut ctx = FloatContext::new();
        // Left float from 0..50 at y=0..100
        ctx.add_left_float(Rect::new(0.0, 0.0, 50.0, 100.0));

        // Place second left float of width 40 at y=0: fits next to first float at x=50
        let (x, y) = ctx.find_float_position(false, 0.0, 40.0, 30.0, 0.0, 200.0);
        assert_eq!(x, 50.0);
        assert_eq!(y, 0.0);

        // Place right float of width 50 at y=0: placed at right edge 200 - 50 = 150
        let (rx, ry) = ctx.find_float_position(true, 0.0, 50.0, 30.0, 0.0, 200.0);
        assert_eq!(rx, 150.0);
        assert_eq!(ry, 0.0);
    }

    #[test]
    fn test_find_float_position_collision_step_down() {
        let mut ctx = FloatContext::new();
        // Left float 0..120, y=0..50 in 200px container
        ctx.add_left_float(Rect::new(0.0, 0.0, 120.0, 50.0));

        // Second float of width 100 cannot fit in remaining 80px, must step down to y=50
        let (x, y) = ctx.find_float_position(false, 0.0, 100.0, 40.0, 0.0, 200.0);
        assert_eq!(y, 50.0);
        assert_eq!(x, 0.0);
    }

    #[test]
    fn test_max_bottom_and_has_floats() {
        let mut ctx = FloatContext::new();
        assert!(!ctx.has_floats());
        assert_eq!(ctx.max_bottom(), None);

        ctx.add_left_float(Rect::new(0.0, 10.0, 50.0, 60.0)); // bottom = 70
        ctx.add_right_float(Rect::new(150.0, 20.0, 50.0, 90.0)); // bottom = 110

        assert!(ctx.has_floats());
        assert_eq!(ctx.left_bottom(), Some(70.0));
        assert_eq!(ctx.right_bottom(), Some(110.0));
        assert_eq!(ctx.max_bottom(), Some(110.0));
    }

    #[test]
    fn test_left_and_right_float_offsets() {
        let mut ctx = FloatContext::new();
        ctx.add_left_float(Rect::new(0.0, 0.0, 40.0, 50.0));
        ctx.add_right_float(Rect::new(160.0, 0.0, 40.0, 50.0));

        assert_eq!(ctx.left_float_offset(10.0, 20.0, 0.0), 40.0);
        assert_eq!(ctx.right_float_offset(10.0, 20.0, 0.0, 200.0), 40.0);
        assert!(ctx.has_floats_at(10.0, 20.0));
        assert!(!ctx.has_floats_at(60.0, 20.0));
    }

    #[test]
    fn test_apply_clearance_with_margin() {
        let mut ctx = FloatContext::new();
        ctx.add_left_float(Rect::new(0.0, 0.0, 50.0, 100.0)); // bottom = 100

        // If current_y = 50 and margin_top = 20, hypothetical border-top is 70 < 100 -> clearance
        let (y, has_clear) = ctx.apply_clearance_with_margin(50.0, 20.0, Clear::Left);
        assert!(has_clear);
        assert_eq!(y, 100.0);

        // If current_y = 90 and margin_top = 20, hypothetical border-top is 110 >= 100 -> no clearance
        let (y2, has_clear2) = ctx.apply_clearance_with_margin(90.0, 20.0, Clear::Left);
        assert!(!has_clear2);
        assert_eq!(y2, 90.0);
    }

    #[test]
    fn test_total_bounds() {
        let mut ctx = FloatContext::new();
        assert_eq!(ctx.total_bounds(), None);

        ctx.add_left_float(Rect::new(10.0, 20.0, 50.0, 60.0)); // 10..60, 20..80
        ctx.add_right_float(Rect::new(150.0, 30.0, 40.0, 70.0)); // 150..190, 30..100

        let bounds = ctx.total_bounds().unwrap();
        assert_eq!(bounds.x(), 10.0);
        assert_eq!(bounds.y(), 20.0);
        assert_eq!(bounds.right(), 190.0);
        assert_eq!(bounds.bottom(), 100.0);
    }
}
