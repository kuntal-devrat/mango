use mango_core::geometry::Rect;

/// Represents scrollable and visual (ink) overflow bounds conforming to CSS Overflow 3
/// and Blink's `LayoutBox::ScrollableOverflowRect` / `VisualOverflowRect`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OverflowModel {
    /// Bounding rectangle for scrollable content, used to compute scroll extents.
    pub scrollable_overflow: Rect,
    /// Bounding rectangle for ink overflow, including box-shadows, outlines, and border decorations.
    pub visual_overflow: Rect,
}

impl OverflowModel {
    /// Creates an overflow model initialized to the box's border rectangle.
    pub fn new(border_box: Rect) -> Self {
        Self {
            scrollable_overflow: border_box,
            visual_overflow: border_box,
        }
    }

    /// Unites a child's scrollable overflow rect into this box's scrollable overflow.
    pub fn unite_scrollable(&mut self, child_rect: Rect) {
        if child_rect.width() <= 0.0 || child_rect.height() <= 0.0 {
            return;
        }
        let min_x = self.scrollable_overflow.x().min(child_rect.x());
        let min_y = self.scrollable_overflow.y().min(child_rect.y());
        let max_x = self.scrollable_overflow.right().max(child_rect.right());
        let max_y = self.scrollable_overflow.bottom().max(child_rect.bottom());
        self.scrollable_overflow = Rect::new(min_x, min_y, max_x - min_x, max_y - min_y);
    }

    /// Unites a visual decoration (outline, shadow) into this box's visual overflow.
    pub fn unite_visual(&mut self, rect: Rect) {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        let min_x = self.visual_overflow.x().min(rect.x());
        let min_y = self.visual_overflow.y().min(rect.y());
        let max_x = self.visual_overflow.right().max(rect.right());
        let max_y = self.visual_overflow.bottom().max(rect.bottom());
        self.visual_overflow = Rect::new(min_x, min_y, max_x - min_x, max_y - min_y);
    }

    /// Returns `true` if children overflow beyond the border box in either dimension.
    pub fn has_scrollable_overflow(&self, border_box: Rect) -> bool {
        self.scrollable_overflow.right() > border_box.right() + 0.5
            || self.scrollable_overflow.bottom() > border_box.bottom() + 0.5
            || self.scrollable_overflow.x() < border_box.x() - 0.5
            || self.scrollable_overflow.y() < border_box.y() - 0.5
    }

    /// Returns the scrollable extent `(max_x - right, max_y - bottom)` extending beyond the border box.
    pub fn scrollable_extent(&self, border_box: Rect) -> (f32, f32) {
        let ext_x = (self.scrollable_overflow.right() - border_box.right()).max(0.0);
        let ext_y = (self.scrollable_overflow.bottom() - border_box.bottom()).max(0.0);
        (ext_x, ext_y)
    }
}
