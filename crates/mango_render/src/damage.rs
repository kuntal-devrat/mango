//! Damage region tracking and dirty-rectangle invalidation.
//!
//! Tracks regions of the screen or layer that require repainting, preventing
//! unnecessary whole-screen re-rasterization when only a portion of the UI
//! or document has changed (e.g. cursor blinking, hover state transitions,
//! localized text mutations, or CSS animations).

use mango_core::Rect;

/// Maximum number of distinct rectangles to track before coalescing into
/// their consolidated bounding box to avoid quadratic intersection overhead.
pub const MAX_DAMAGE_RECTS: usize = 16;

/// A collection of 2D rectangles representing damaged (dirty) areas.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DamageRegion {
    rects: Vec<Rect>,
}

impl DamageRegion {
    /// Creates a new, empty damage region.
    pub fn new() -> Self {
        Self { rects: Vec::new() }
    }

    /// Creates a damage region containing a single damaged rectangle.
    pub fn from_rect(rect: Rect) -> Self {
        let mut r = Self::new();
        r.add_rect(rect);
        r
    }

    /// Creates a damage region representing full window/viewport damage.
    pub fn full(width: f32, height: f32) -> Self {
        Self::from_rect(Rect::new(0.0, 0.0, width.max(0.0), height.max(0.0)))
    }

    /// Returns `true` if there are no damaged areas.
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    /// Returns the number of distinct damaged rectangles.
    pub fn len(&self) -> usize {
        self.rects.len()
    }

    /// Clears all damaged areas.
    pub fn clear(&mut self) {
        self.rects.clear();
    }

    /// Returns a slice of all tracked damage rectangles.
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// Consumes the damage region, returning its underlying vector of rectangles.
    pub fn into_rects(self) -> Vec<Rect> {
        self.rects
    }

    /// Adds a rectangle to the damaged region.
    ///
    /// Ignores rectangles with zero or negative dimensions. If adding the rectangle
    /// causes the count to exceed [`MAX_DAMAGE_RECTS`], the region is coalesced.
    pub fn add_rect(&mut self, rect: Rect) {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }

        // If the rectangle is fully enclosed by an existing damaged rect, no-op.
        for existing in &self.rects {
            if existing.x() <= rect.x()
                && existing.y() <= rect.y()
                && existing.x() + existing.width() >= rect.x() + rect.width()
                && existing.y() + existing.height() >= rect.y() + rect.height()
            {
                return;
            }
        }

        // If the new rectangle completely encloses any existing rects, remove them.
        self.rects.retain(|existing| {
            !(rect.x() <= existing.x()
                && rect.y() <= existing.y()
                && rect.x() + rect.width() >= existing.x() + existing.width()
                && rect.y() + rect.height() >= existing.y() + existing.height())
        });

        self.rects.push(rect);

        if self.rects.len() > MAX_DAMAGE_RECTS {
            self.coalesce();
        }
    }

    /// Merges another damage region into this one.
    pub fn add_damage(&mut self, other: &DamageRegion) {
        for rect in &other.rects {
            self.add_rect(*rect);
        }
    }

    /// Returns the consolidated axis-aligned bounding box enclosing all damaged rectangles,
    /// or `None` if the region is empty.
    pub fn bounds(&self) -> Option<Rect> {
        if self.rects.is_empty() {
            return None;
        }

        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for r in &self.rects {
            min_x = min_x.min(r.x());
            min_y = min_y.min(r.y());
            max_x = max_x.max(r.x() + r.width());
            max_y = max_y.max(r.y() + r.height());
        }

        if min_x < max_x && min_y < max_y {
            Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
        } else {
            None
        }
    }

    /// Returns `true` if any damaged rectangle intersects `target`.
    pub fn intersects(&self, target: Rect) -> bool {
        if target.width() <= 0.0 || target.height() <= 0.0 {
            return false;
        }
        for r in &self.rects {
            let x1 = r.x().max(target.x());
            let y1 = r.y().max(target.y());
            let x2 = (r.x() + r.width()).min(target.x() + target.width());
            let y2 = (r.y() + r.height()).min(target.y() + target.height());
            if x2 > x1 && y2 > y1 {
                return true;
            }
        }
        false
    }

    /// Returns `true` if the overall bounding box of the damage region intersects `target`.
    pub fn bounds_intersects(&self, target: Rect) -> bool {
        if let Some(b) = self.bounds() {
            let x1 = b.x().max(target.x());
            let y1 = b.y().max(target.y());
            let x2 = (b.x() + b.width()).min(target.x() + target.width());
            let y2 = (b.y() + b.height()).min(target.y() + target.height());
            x2 > x1 && y2 > y1
        } else {
            false
        }
    }

    /// Restricts all damaged rectangles to the specified boundary (e.g. viewport or layer rect).
    /// Rectangles outside the boundary are dropped.
    pub fn clamp_to(&mut self, bounds: Rect) {
        let mut clamped = Vec::with_capacity(self.rects.len());
        for r in &self.rects {
            let x1 = r.x().max(bounds.x());
            let y1 = r.y().max(bounds.y());
            let x2 = (r.x() + r.width()).min(bounds.x() + bounds.width());
            let y2 = (r.y() + r.height()).min(bounds.y() + bounds.height());
            let w = x2 - x1;
            let h = y2 - y1;
            if w > 0.0 && h > 0.0 {
                clamped.push(Rect::new(x1, y1, w, h));
            }
        }
        self.rects = clamped;
    }

    /// Coalesces overlapping or nearby rectangles to reduce total rectangle count.
    pub fn coalesce(&mut self) {
        if self.rects.len() <= 1 {
            return;
        }

        let mut changed = true;
        while changed && self.rects.len() > 1 {
            changed = false;
            let mut i = 0;
            while i < self.rects.len() {
                let mut j = i + 1;
                while j < self.rects.len() {
                    let a = self.rects[i];
                    let b = self.rects[j];

                    let union_r = a.union(&b);
                    let area_a = a.width() * a.height();
                    let area_b = b.width() * b.height();
                    let union_area = union_r.width() * union_r.height();

                    // If union adds less than 35% empty space, or if they overlap, merge them.
                    let overlap = a.x() < b.x() + b.width()
                        && a.x() + a.width() > b.x()
                        && a.y() < b.y() + b.height()
                        && a.y() + a.height() > b.y();

                    if overlap || union_area <= (area_a + area_b) * 1.35 {
                        self.rects[i] = union_r;
                        self.rects.swap_remove(j);
                        changed = true;
                    } else {
                        j += 1;
                    }
                }
                i += 1;
            }
        }

        // If still exceeding MAX_DAMAGE_RECTS, consolidate everything to the total bounding box.
        if self.rects.len() > MAX_DAMAGE_RECTS {
            self.union_all();
        }
    }

    /// Replaces all damaged rectangles with their single consolidated bounding box.
    pub fn union_all(&mut self) {
        if let Some(b) = self.bounds() {
            self.rects = vec![b];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_region_empty() {
        let region = DamageRegion::new();
        assert!(region.is_empty());
        assert_eq!(region.len(), 0);
        assert_eq!(region.bounds(), None);
        assert!(!region.intersects(Rect::new(0.0, 0.0, 100.0, 100.0)));
    }

    #[test]
    fn test_damage_region_add_and_bounds() {
        let mut region = DamageRegion::new();
        region.add_rect(Rect::new(10.0, 20.0, 30.0, 40.0));
        assert!(!region.is_empty());
        assert_eq!(region.len(), 1);
        assert_eq!(region.bounds(), Some(Rect::new(10.0, 20.0, 30.0, 40.0)));

        region.add_rect(Rect::new(50.0, 80.0, 20.0, 20.0));
        assert_eq!(region.len(), 2);
        assert_eq!(region.bounds(), Some(Rect::new(10.0, 20.0, 60.0, 80.0)));
    }

    #[test]
    fn test_damage_region_enclosure_dedup() {
        let mut region = DamageRegion::new();
        region.add_rect(Rect::new(0.0, 0.0, 100.0, 100.0));
        // Sub-rectangle enclosed inside existing rect should not increase length
        region.add_rect(Rect::new(10.0, 10.0, 20.0, 20.0));
        assert_eq!(region.len(), 1);

        // Larger rectangle enclosing existing should replace it
        region.add_rect(Rect::new(0.0, 0.0, 200.0, 200.0));
        assert_eq!(region.len(), 1);
        assert_eq!(region.rects()[0], Rect::new(0.0, 0.0, 200.0, 200.0));
    }

    #[test]
    fn test_damage_region_clamp() {
        let mut region = DamageRegion::new();
        region.add_rect(Rect::new(-10.0, -10.0, 50.0, 50.0));
        region.add_rect(Rect::new(500.0, 500.0, 100.0, 100.0));

        let viewport = Rect::new(0.0, 0.0, 100.0, 100.0);
        region.clamp_to(viewport);

        assert_eq!(region.len(), 1);
        assert_eq!(region.rects()[0], Rect::new(0.0, 0.0, 40.0, 40.0));
    }

    #[test]
    fn test_damage_region_coalesce_overlapping() {
        let mut region = DamageRegion::new();
        region.add_rect(Rect::new(0.0, 0.0, 50.0, 50.0));
        region.add_rect(Rect::new(40.0, 0.0, 50.0, 50.0)); // overlaps horizontally
        region.coalesce();

        assert_eq!(region.len(), 1);
        assert_eq!(region.bounds(), Some(Rect::new(0.0, 0.0, 90.0, 50.0)));
    }

    #[test]
    fn test_damage_region_intersects() {
        let mut region = DamageRegion::new();
        region.add_rect(Rect::new(10.0, 10.0, 20.0, 20.0));

        assert!(region.intersects(Rect::new(15.0, 15.0, 10.0, 10.0)));
        assert!(!region.intersects(Rect::new(50.0, 50.0, 10.0, 10.0)));
    }
}
