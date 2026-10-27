use mango_core::Point;

use super::types::LayoutBox;

impl LayoutBox {
    /// Returns true if this box acts as a scroll container (PRD 8.1 / CSS Overflow 3 §3).
    pub fn is_scroll_container(&self) -> bool {
        if let Some(s) = &self.style {
            matches!(
                s.overflow_x,
                mango_css::values::Overflow::Scroll
                    | mango_css::values::Overflow::Auto
                    | mango_css::values::Overflow::Hidden
            ) || matches!(
                s.overflow_y,
                mango_css::values::Overflow::Scroll
                    | mango_css::values::Overflow::Auto
                    | mango_css::values::Overflow::Hidden
            )
        } else {
            false
        }
    }

    /// Returns true if this box can be scrolled interactively by the user via mouse wheel or scrollbars.
    #[inline]
    pub fn is_user_scrollable(&self) -> bool {
        if let Some(s) = &self.style {
            let can_user = matches!(
                s.overflow_x,
                mango_css::values::Overflow::Scroll | mango_css::values::Overflow::Auto
            ) || matches!(
                s.overflow_y,
                mango_css::values::Overflow::Scroll | mango_css::values::Overflow::Auto
            );
            if !can_user {
                return false;
            }
        } else {
            return false;
        }
        let (max_x, max_y) = self.max_scroll();
        max_x > 0.0 || max_y > 0.0
    }

    /// Computes the bounding extent of all in-flow child boxes relative to padding box origin.
    ///
    /// Implements CSS Overflow 3 §3.2 and CSSOM View scrollWidth/scrollHeight:
    /// includes in-flow descendants and positioned descendants whose containing block is this scroll container.
    pub fn scrollable_extent(&self) -> (f32, f32) {
        let pad_box = self.dimensions.padding_box();
        let mut max_x = pad_box.width();
        let mut max_y = pad_box.height();
        let pad_origin_x = pad_box.x();
        let pad_origin_y = pad_box.y();

        let self_is_cb = self
            .style
            .as_ref()
            .is_some_and(|s| s.position != mango_css::values::Position::Static);

        for child in &self.children {
            let is_fixed = child
                .style
                .as_ref()
                .is_some_and(|s| s.position == mango_css::values::Position::Fixed);
            let is_absolute = child
                .style
                .as_ref()
                .is_some_and(|s| s.position == mango_css::values::Position::Absolute);

            if is_fixed || (is_absolute && !self_is_cb) {
                continue;
            }

            let is_rtl_x = self.style.as_ref().is_some_and(|s| {
                s.writing_mode == mango_css::values::WritingMode::VerticalRl
                    || s.direction == mango_css::values::Direction::Rtl
            });

            let is_rel_pos = child
                .style
                .as_ref()
                .is_some_and(|s| s.position == mango_css::values::Position::Relative);
            let is_in_flow = !is_absolute && !is_fixed;
            let (pad_end_x, pad_end_y) = if is_in_flow && !is_rel_pos {
                let px = if is_rtl_x {
                    self.dimensions.padding.left
                } else {
                    self.dimensions.padding.right
                };
                (px, self.dimensions.padding.bottom)
            } else {
                (0.0, 0.0)
            };

            let bb = child.dimensions.border_box();
            let mb = child.dimensions.margin_box();
            if is_rtl_x {
                max_x = max_x
                    .max(pad_box.right() - bb.left() + pad_end_x)
                    .max(pad_box.right() - mb.left() + pad_end_x);
            } else {
                max_x = max_x
                    .max(bb.right() - pad_origin_x + pad_end_x)
                    .max(mb.right() - pad_origin_x + pad_end_x);
            }
            max_y = max_y
                .max(bb.bottom() - pad_origin_y + pad_end_y)
                .max(mb.bottom() - pad_origin_y + pad_end_y);

            // If child has visible overflow, its overflowing contents expand this container's scroll extent
            let clips = child.is_scroll_container()
                || child.style.as_ref().is_some_and(|s| {
                    s.overflow_x != mango_css::values::Overflow::Visible
                        || s.overflow_y != mango_css::values::Overflow::Visible
                });
            if !clips && !child.children.is_empty() {
                let (c_ext_x, c_ext_y) = child.scrollable_extent();
                let child_pad_x = child.dimensions.padding_box().x();
                let child_pad_y = child.dimensions.padding_box().y();
                max_x = max_x.max((child_pad_x + c_ext_x) - pad_origin_x + pad_end_x);
                max_y = max_y.max((child_pad_y + c_ext_y) - pad_origin_y + pad_end_y);
            }
        }
        (max_x, max_y)
    }

    /// Returns the maximum allowed scroll offsets (max_scroll_x, max_scroll_y).
    pub fn max_scroll(&self) -> (f32, f32) {
        let (extent_x, extent_y) = self.scrollable_extent();
        let pad_box = self.dimensions.padding_box();
        let max_x = (extent_x - pad_box.width()).max(0.0);
        let max_y = (extent_y - pad_box.height()).max(0.0);
        (max_x, max_y)
    }

    /// Scrolls the container by (delta_x, delta_y), returning the delta consumed.
    pub fn scroll_by(&mut self, delta_x: f32, delta_y: f32) -> (f32, f32) {
        let (max_x, max_y) = self.max_scroll();
        let old_x = self.scroll_offset_x;
        let old_y = self.scroll_offset_y;

        let is_rtl_x = self.style.as_ref().is_some_and(|s| {
            s.writing_mode == mango_css::values::WritingMode::VerticalRl
                || s.direction == mango_css::values::Direction::Rtl
        });
        if is_rtl_x {
            self.scroll_offset_x = (self.scroll_offset_x + delta_x).clamp(-max_x, 0.0);
        } else {
            self.scroll_offset_x = (self.scroll_offset_x + delta_x).clamp(0.0, max_x);
        }
        self.scroll_offset_y = (self.scroll_offset_y + delta_y).clamp(0.0, max_y);

        (self.scroll_offset_x - old_x, self.scroll_offset_y - old_y)
    }

    /// Finds the innermost scrollable container that contains the point `(x, y)`.
    pub fn find_scrollable_container_at(&self, x: f32, y: f32) -> Option<&LayoutBox> {
        let pad_box = self.dimensions.padding_box();
        if !pad_box.contains(Point::new(x, y)) {
            return None;
        }

        let (query_x, query_y) = if self.is_scroll_container() {
            (x + self.scroll_offset_x, y + self.scroll_offset_y)
        } else {
            (x, y)
        };

        for child in self.children.iter().rev() {
            let (child_x, child_y) = if child
                .style
                .as_ref()
                .is_some_and(|s| s.position == mango_css::values::Position::Fixed)
            {
                (x, y)
            } else {
                (query_x, query_y)
            };
            if let Some(found) = child.find_scrollable_container_at(child_x, child_y) {
                return Some(found);
            }
        }

        if self.is_scroll_container() {
            Some(self)
        } else {
            None
        }
    }

    /// Scrolls the innermost container at (x, y) by (delta_x, delta_y).
    /// If that container cannot consume the entire delta (reaches boundary),
    /// propagates the remainder up to outer nested scroll containers.
    /// Returns the total unconsumed (delta_x, delta_y) that can be applied to page scroll.
    pub fn dispatch_nested_scroll(
        &mut self,
        x: f32,
        y: f32,
        mut delta_x: f32,
        mut delta_y: f32,
    ) -> (f32, f32) {
        let pad_box = self.dimensions.padding_box();
        if !pad_box.contains(Point::new(x, y)) {
            return (delta_x, delta_y);
        }

        let (query_x, query_y) = if self.is_scroll_container() {
            (x + self.scroll_offset_x, y + self.scroll_offset_y)
        } else {
            (x, y)
        };

        // Recurse into children first (innermost container gets first opportunity)
        for child in self.children.iter_mut().rev() {
            let (child_x, child_y) = if child
                .style
                .as_ref()
                .is_some_and(|s| s.position == mango_css::values::Position::Fixed)
            {
                (x, y)
            } else {
                (query_x, query_y)
            };
            let (rem_x, rem_y) = child.dispatch_nested_scroll(child_x, child_y, delta_x, delta_y);
            delta_x = rem_x;
            delta_y = rem_y;
            if delta_x == 0.0 && delta_y == 0.0 {
                return (0.0, 0.0);
            }
        }

        // If this box is a scroll container, consume whatever delta remains
        if self.is_scroll_container() {
            let (consumed_x, consumed_y) = self.scroll_by(delta_x, delta_y);
            delta_x -= consumed_x;
            delta_y -= consumed_y;
        }

        (delta_x, delta_y)
    }
}
