//! Box dimensions and geometric representations for the CSS box model.

use mango_core::{EdgeSizes, Point, Rect};
use mango_css::values::WritingMode;

/// Full dimensional properties of a layout box, conforming to the CSS box model.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Dimensions {
    /// Content area rectangle (relative to document origin).
    pub content: Rect,
    /// Padding edge sizes.
    pub padding: EdgeSizes,
    /// Border edge sizes.
    pub border: EdgeSizes,
    /// Margin edge sizes.
    pub margin: EdgeSizes,
}

impl Dimensions {
    pub const ZERO: Dimensions = Dimensions {
        content: Rect::ZERO,
        padding: EdgeSizes::ZERO,
        border: EdgeSizes::ZERO,
        margin: EdgeSizes::ZERO,
    };

    #[inline]
    pub const fn new(content: Rect) -> Self {
        Self {
            content,
            padding: EdgeSizes::ZERO,
            border: EdgeSizes::ZERO,
            margin: EdgeSizes::ZERO,
        }
    }

    /// Constructs a [`Dimensions`] instance directly from a content box rectangle and all edge sizes.
    #[inline]
    pub const fn from_content_box(
        content: Rect,
        padding: EdgeSizes,
        border: EdgeSizes,
        margin: EdgeSizes,
    ) -> Self {
        Self {
            content,
            padding,
            border,
            margin,
        }
    }

    /// The content area box.
    #[inline]
    pub fn content_box(&self) -> Rect {
        self.content
    }

    /// The padding box (content area + padding).
    #[inline]
    pub fn padding_box(&self) -> Rect {
        Rect::new(
            self.content.x() - self.padding.left,
            self.content.y() - self.padding.top,
            (self.content.width() + self.padding.horizontal()).max(0.0),
            (self.content.height() + self.padding.vertical()).max(0.0),
        )
    }

    /// The border box (padding box + border).
    #[inline]
    pub fn border_box(&self) -> Rect {
        let pad = self.padding_box();
        Rect::new(
            pad.x() - self.border.left,
            pad.y() - self.border.top,
            (pad.width() + self.border.horizontal()).max(0.0),
            (pad.height() + self.border.vertical()).max(0.0),
        )
    }

    /// The margin box (border box + margin).
    ///
    /// Per CSS 2.1 §8.3 and CSS Box Model Level 3/4, physical rectangles cannot have
    /// negative width or height. If large negative margins exceed the border box dimensions,
    /// the width and height clamp to `0.0`.
    #[inline]
    pub fn margin_box(&self) -> Rect {
        let b = self.border_box();
        Rect::new(
            b.x() - self.margin.left,
            b.y() - self.margin.top,
            (b.width() + self.margin.horizontal()).max(0.0),
            (b.height() + self.margin.vertical()).max(0.0),
        )
    }

    /// Returns the border box with coordinates and dimensions rounded to device pixels (Chromium Blink `PixelSnappedBorderBoxRect`).
    #[inline]
    pub fn pixel_snapped_border_box(&self) -> Rect {
        let b = self.border_box();
        Rect::new(
            b.x().round(),
            b.y().round(),
            b.width().round().max(0.0),
            b.height().round().max(0.0),
        )
    }

    /// Returns the content box with coordinates and dimensions rounded to device pixels.
    #[inline]
    pub fn pixel_snapped_content_box(&self) -> Rect {
        Rect::new(
            self.content.x().round(),
            self.content.y().round(),
            self.content.width().round().max(0.0),
            self.content.height().round().max(0.0),
        )
    }

    /// Returns the padding box with coordinates and dimensions rounded to device pixels.
    #[inline]
    pub fn pixel_snapped_padding_box(&self) -> Rect {
        let p = self.padding_box();
        Rect::new(
            p.x().round(),
            p.y().round(),
            p.width().round().max(0.0),
            p.height().round().max(0.0),
        )
    }

    /// Translates this box by `(dx, dy)`.
    #[inline]
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.content.origin.x += dx;
        self.content.origin.y += dy;
    }

    /// Returns a new `Dimensions` translated by `(dx, dy)`.
    #[inline]
    pub fn translated(&self, dx: f32, dy: f32) -> Self {
        let mut d = *self;
        d.translate(dx, dy);
        d
    }

    /// Offsets the content origin by a given `Point`.
    #[inline]
    pub fn offset_by(&mut self, point: Point) {
        self.translate(point.x, point.y);
    }

    /// Constructs a [`Dimensions`] instance from a specified border box rectangle and edge sizes.
    pub fn from_border_box(
        border_box: Rect,
        padding: EdgeSizes,
        border: EdgeSizes,
        margin: EdgeSizes,
    ) -> Self {
        let content_x = border_box.x() + border.left + padding.left;
        let content_y = border_box.y() + border.top + padding.top;
        let content_w = (border_box.width() - border.horizontal() - padding.horizontal()).max(0.0);
        let content_h = (border_box.height() - border.vertical() - padding.vertical()).max(0.0);

        Self {
            content: Rect::new(content_x, content_y, content_w, content_h),
            padding,
            border,
            margin,
        }
    }

    /// Content inline size according to writing mode (CSS Writing Modes Level 3 §6).
    #[inline]
    pub fn inline_size(&self, wm: WritingMode) -> f32 {
        match wm {
            WritingMode::HorizontalTb => self.content.width(),
            WritingMode::VerticalRl | WritingMode::VerticalLr => self.content.height(),
        }
    }

    /// Content block size according to writing mode (CSS Writing Modes Level 3 §6).
    #[inline]
    pub fn block_size(&self, wm: WritingMode) -> f32 {
        match wm {
            WritingMode::HorizontalTb => self.content.height(),
            WritingMode::VerticalRl | WritingMode::VerticalLr => self.content.width(),
        }
    }

    /// Padding box inline size according to writing mode.
    #[inline]
    pub fn padding_box_inline_size(&self, wm: WritingMode) -> f32 {
        let p = self.padding_box();
        match wm {
            WritingMode::HorizontalTb => p.width(),
            WritingMode::VerticalRl | WritingMode::VerticalLr => p.height(),
        }
    }

    /// Padding box block size according to writing mode.
    #[inline]
    pub fn padding_box_block_size(&self, wm: WritingMode) -> f32 {
        let p = self.padding_box();
        match wm {
            WritingMode::HorizontalTb => p.height(),
            WritingMode::VerticalRl | WritingMode::VerticalLr => p.width(),
        }
    }

    /// Border box inline size according to writing mode.
    #[inline]
    pub fn border_box_inline_size(&self, wm: WritingMode) -> f32 {
        let b = self.border_box();
        match wm {
            WritingMode::HorizontalTb => b.width(),
            WritingMode::VerticalRl | WritingMode::VerticalLr => b.height(),
        }
    }

    /// Border box block size according to writing mode.
    #[inline]
    pub fn border_box_block_size(&self, wm: WritingMode) -> f32 {
        let b = self.border_box();
        match wm {
            WritingMode::HorizontalTb => b.height(),
            WritingMode::VerticalRl | WritingMode::VerticalLr => b.width(),
        }
    }

    /// Margin box inline size according to writing mode.
    #[inline]
    pub fn margin_box_inline_size(&self, wm: WritingMode) -> f32 {
        let m = self.margin_box();
        match wm {
            WritingMode::HorizontalTb => m.width(),
            WritingMode::VerticalRl | WritingMode::VerticalLr => m.height(),
        }
    }

    /// Margin box block size according to writing mode.
    #[inline]
    pub fn margin_box_block_size(&self, wm: WritingMode) -> f32 {
        let m = self.margin_box();
        match wm {
            WritingMode::HorizontalTb => m.height(),
            WritingMode::VerticalRl | WritingMode::VerticalLr => m.width(),
        }
    }

    /// Non-content spacing on the inline axis (`padding + border`).
    #[inline]
    pub fn non_content_inline(&self, wm: WritingMode) -> f32 {
        match wm {
            WritingMode::HorizontalTb => self.non_content_horizontal(),
            WritingMode::VerticalRl | WritingMode::VerticalLr => self.non_content_vertical(),
        }
    }

    /// Non-content spacing on the block axis (`padding + border`).
    #[inline]
    pub fn non_content_block(&self, wm: WritingMode) -> f32 {
        match wm {
            WritingMode::HorizontalTb => self.non_content_vertical(),
            WritingMode::VerticalRl | WritingMode::VerticalLr => self.non_content_horizontal(),
        }
    }

    /// Total edges on the inline axis (`padding + border + margin`).
    #[inline]
    pub fn total_inline_edges(&self, wm: WritingMode) -> f32 {
        match wm {
            WritingMode::HorizontalTb => self.total_horizontal_edges(),
            WritingMode::VerticalRl | WritingMode::VerticalLr => self.total_vertical_edges(),
        }
    }

    /// Total edges on the block axis (`padding + border + margin`).
    #[inline]
    pub fn total_block_edges(&self, wm: WritingMode) -> f32 {
        match wm {
            WritingMode::HorizontalTb => self.total_vertical_edges(),
            WritingMode::VerticalRl | WritingMode::VerticalLr => self.total_horizontal_edges(),
        }
    }

    /// Returns `true` if content width or height is zero or negative.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.content.width() <= 0.0 || self.content.height() <= 0.0
    }

    /// Returns `true` if content is zero and all edge sizes are zero.
    #[inline]
    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }

    /// Returns total horizontal non-content spacing (`padding.horizontal() + border.horizontal()`).
    #[inline]
    pub fn non_content_horizontal(&self) -> f32 {
        self.padding.horizontal() + self.border.horizontal()
    }

    /// Returns total vertical non-content spacing (`padding.vertical() + border.vertical()`).
    #[inline]
    pub fn non_content_vertical(&self) -> f32 {
        self.padding.vertical() + self.border.vertical()
    }

    /// Returns total horizontal edges (`padding + border + margin`).
    #[inline]
    pub fn total_horizontal_edges(&self) -> f32 {
        self.padding.horizontal() + self.border.horizontal() + self.margin.horizontal()
    }

    /// Returns total vertical edges (`padding + border + margin`).
    #[inline]
    pub fn total_vertical_edges(&self) -> f32 {
        self.padding.vertical() + self.border.vertical() + self.margin.vertical()
    }

    /// Returns the rectangle corresponding to the given concentric box area.
    #[inline]
    pub fn area_rect(&self, area: crate::box_model::BoxArea) -> Rect {
        match area {
            crate::box_model::BoxArea::Content => self.content_box(),
            crate::box_model::BoxArea::Padding => self.padding_box(),
            crate::box_model::BoxArea::Border => self.border_box(),
            crate::box_model::BoxArea::Margin => self.margin_box(),
        }
    }

    /// Expands a rectangle outward by specified edge sizes.
    #[inline]
    pub fn outset_by(rect: Rect, edges: EdgeSizes) -> Rect {
        Rect::new(
            rect.x() - edges.left,
            rect.y() - edges.top,
            (rect.width() + edges.horizontal()).max(0.0),
            (rect.height() + edges.vertical()).max(0.0),
        )
    }

    /// Insets a rectangle inward by specified edge sizes.
    #[inline]
    pub fn inset_by(rect: Rect, edges: EdgeSizes) -> Rect {
        Rect::new(
            rect.x() + edges.left,
            rect.y() + edges.top,
            (rect.width() - edges.horizontal()).max(0.0),
            (rect.height() - edges.vertical()).max(0.0),
        )
    }

    /// Hit-tests a point against the concentric box model areas.
    ///
    /// Returns `Some(BoxArea)` representing the most specific (innermost) area containing the point,
    /// or `None` if the point falls outside the margin box.
    pub fn hit_test(&self, point: mango_core::Point) -> Option<crate::box_model::BoxArea> {
        if self.content_box().contains(point) {
            Some(crate::box_model::BoxArea::Content)
        } else if self.padding_box().contains(point) {
            Some(crate::box_model::BoxArea::Padding)
        } else if self.border_box().contains(point) {
            Some(crate::box_model::BoxArea::Border)
        } else if self.margin_box().contains(point) {
            Some(crate::box_model::BoxArea::Margin)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimensions_boxes() {
        let mut dims = Dimensions::new(Rect::new(10.0, 20.0, 100.0, 50.0));
        dims.padding = EdgeSizes::all(5.0);
        dims.border = EdgeSizes::all(2.0);
        dims.margin = EdgeSizes::all(10.0);

        // Content
        assert_eq!(dims.content_box(), Rect::new(10.0, 20.0, 100.0, 50.0));

        // Padding box: x=10-5=5, y=20-5=15, w=100+10=110, h=50+10=60
        assert_eq!(dims.padding_box(), Rect::new(5.0, 15.0, 110.0, 60.0));

        // Border box: x=5-2=3, y=15-2=13, w=110+4=114, h=60+4=64
        assert_eq!(dims.border_box(), Rect::new(3.0, 13.0, 114.0, 64.0));

        // Margin box: x=3-10=-7, y=13-10=3, w=114+20=134, h=64+20=84
        assert_eq!(dims.margin_box(), Rect::new(-7.0, 3.0, 134.0, 84.0));

        // Edge sums
        assert_eq!(dims.non_content_horizontal(), 14.0);
        assert_eq!(dims.non_content_vertical(), 14.0);
        assert_eq!(dims.total_horizontal_edges(), 34.0);
        assert_eq!(dims.total_vertical_edges(), 34.0);

        // Hit testing
        assert_eq!(
            dims.hit_test(mango_core::Point::new(50.0, 40.0)),
            Some(crate::box_model::BoxArea::Content)
        );
        assert_eq!(
            dims.hit_test(mango_core::Point::new(6.0, 16.0)),
            Some(crate::box_model::BoxArea::Padding)
        );
        assert_eq!(
            dims.hit_test(mango_core::Point::new(4.0, 14.0)),
            Some(crate::box_model::BoxArea::Border)
        );
        assert_eq!(
            dims.hit_test(mango_core::Point::new(-2.0, 5.0)),
            Some(crate::box_model::BoxArea::Margin)
        );
        assert_eq!(dims.hit_test(mango_core::Point::new(-20.0, -20.0)), None);

        // From border box reconstruction
        let reconstructed = Dimensions::from_border_box(
            dims.border_box(),
            dims.padding,
            dims.border,
            dims.margin,
        );
        assert_eq!(reconstructed.content_box(), dims.content_box());
        assert_eq!(reconstructed.padding_box(), dims.padding_box());
        assert_eq!(reconstructed.border_box(), dims.border_box());
        assert_eq!(reconstructed.margin_box(), dims.margin_box());
    }

    #[test]
    fn test_dimensions_negative_margins_clamped() {
        // Border box width 50, height 50. Negative margin with horizontal sum -70, vertical sum -60.
        let dims = Dimensions::from_content_box(
            Rect::new(10.0, 10.0, 50.0, 50.0),
            EdgeSizes::ZERO,
            EdgeSizes::ZERO,
            EdgeSizes::new(-30.0, -40.0, -30.0, -30.0),
        );

        let m = dims.margin_box();
        // x = 10 - (-30) = 40, y = 10 - (-30) = 40
        assert_eq!(m.x(), 40.0);
        assert_eq!(m.y(), 40.0);
        // width = max(0, 50 + (-70)) = 0.0
        assert_eq!(m.width(), 0.0);
        // height = max(0, 50 + (-60)) = 0.0
        assert_eq!(m.height(), 0.0);
    }

    #[test]
    fn test_dimensions_from_content_box() {
        let content = Rect::new(5.0, 10.0, 100.0, 80.0);
        let pad = EdgeSizes::all(4.0);
        let border = EdgeSizes::all(1.0);
        let margin = EdgeSizes::all(8.0);

        let dims = Dimensions::from_content_box(content, pad, border, margin);
        assert_eq!(dims.content, content);
        assert_eq!(dims.padding, pad);
        assert_eq!(dims.border, border);
        assert_eq!(dims.margin, margin);
    }

    #[test]
    fn test_dimensions_translation() {
        let mut dims = Dimensions::new(Rect::new(10.0, 20.0, 50.0, 40.0));
        dims.translate(5.0, -10.0);
        assert_eq!(dims.content.origin, Point::new(15.0, 10.0));

        let translated = dims.translated(-5.0, 10.0);
        assert_eq!(translated.content.origin, Point::new(10.0, 20.0));

        dims.offset_by(Point::new(2.0, 3.0));
        assert_eq!(dims.content.origin, Point::new(17.0, 13.0));
    }

    #[test]
    fn test_dimensions_logical_writing_modes() {
        let dims = Dimensions::from_content_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            EdgeSizes::new(10.0, 20.0, 30.0, 40.0), // top, right, bottom, left (vert=40, horiz=60)
            EdgeSizes::new(2.0, 4.0, 6.0, 8.0),     // top, right, bottom, left (vert=8, horiz=12)
            EdgeSizes::new(5.0, 10.0, 15.0, 20.0),  // top, right, bottom, left (vert=20, horiz=30)
        );

        // Horizontal-Tb: Inline = Horizontal (width), Block = Vertical (height)
        assert_eq!(dims.inline_size(WritingMode::HorizontalTb), 120.0);
        assert_eq!(dims.block_size(WritingMode::HorizontalTb), 80.0);
        assert_eq!(dims.non_content_inline(WritingMode::HorizontalTb), 72.0); // 60 + 12
        assert_eq!(dims.non_content_block(WritingMode::HorizontalTb), 48.0);  // 40 + 8
        assert_eq!(dims.total_inline_edges(WritingMode::HorizontalTb), 102.0); // 60 + 12 + 30
        assert_eq!(dims.total_block_edges(WritingMode::HorizontalTb), 68.0);   // 40 + 8 + 20

        // Vertical-Rl: Inline = Vertical (height), Block = Horizontal (width)
        assert_eq!(dims.inline_size(WritingMode::VerticalRl), 80.0);
        assert_eq!(dims.block_size(WritingMode::VerticalRl), 120.0);
        assert_eq!(dims.non_content_inline(WritingMode::VerticalRl), 48.0);
        assert_eq!(dims.non_content_block(WritingMode::VerticalRl), 72.0);
        assert_eq!(dims.total_inline_edges(WritingMode::VerticalRl), 68.0);
        assert_eq!(dims.total_block_edges(WritingMode::VerticalRl), 102.0);

        // Border box dimensions
        let bb = dims.border_box();
        assert_eq!(dims.border_box_inline_size(WritingMode::HorizontalTb), bb.width());
        assert_eq!(dims.border_box_block_size(WritingMode::HorizontalTb), bb.height());
        assert_eq!(dims.border_box_inline_size(WritingMode::VerticalRl), bb.height());
        assert_eq!(dims.border_box_block_size(WritingMode::VerticalRl), bb.width());

        // Margin box dimensions
        let mb = dims.margin_box();
        assert_eq!(dims.margin_box_inline_size(WritingMode::HorizontalTb), mb.width());
        assert_eq!(dims.margin_box_block_size(WritingMode::HorizontalTb), mb.height());
        assert_eq!(dims.margin_box_inline_size(WritingMode::VerticalLr), mb.height());
        assert_eq!(dims.margin_box_block_size(WritingMode::VerticalLr), mb.width());
    }

    #[test]
    fn test_dimensions_pixel_snapping() {
        let dims = Dimensions::from_content_box(
            Rect::new(10.4, 20.6, 99.7, 49.3),
            EdgeSizes::all(5.0),
            EdgeSizes::all(1.0),
            EdgeSizes::ZERO,
        );

        let snapped_content = dims.pixel_snapped_content_box();
        assert_eq!(snapped_content.x(), 10.0);
        assert_eq!(snapped_content.y(), 21.0);
        assert_eq!(snapped_content.width(), 100.0);
        assert_eq!(snapped_content.height(), 49.0);

        let snapped_border = dims.pixel_snapped_border_box();
        let raw_bb = dims.border_box();
        assert_eq!(snapped_border.x(), raw_bb.x().round());
        assert_eq!(snapped_border.y(), raw_bb.y().round());
        assert_eq!(snapped_border.width(), raw_bb.width().round());
        assert_eq!(snapped_border.height(), raw_bb.height().round());
    }

    #[test]
    fn test_dimensions_is_empty_and_is_zero() {
        assert!(Dimensions::ZERO.is_zero());
        assert!(Dimensions::ZERO.is_empty());

        let non_empty = Dimensions::new(Rect::new(0.0, 0.0, 10.0, 10.0));
        assert!(!non_empty.is_empty());
        assert!(!non_empty.is_zero());

        let zero_area = Dimensions::new(Rect::new(5.0, 5.0, 0.0, 20.0));
        assert!(zero_area.is_empty());
        assert!(!zero_area.is_zero());
    }

    #[test]
    fn test_dimensions_outset_and_inset() {
        let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
        let edges = EdgeSizes::all(5.0);

        let outset = Dimensions::outset_by(rect, edges);
        assert_eq!(outset, Rect::new(5.0, 15.0, 110.0, 60.0));

        let inset = Dimensions::inset_by(rect, edges);
        assert_eq!(inset, Rect::new(15.0, 25.0, 90.0, 40.0));

        // Inset larger than size clamps to 0
        let large_edges = EdgeSizes::all(60.0);
        let clamped_inset = Dimensions::inset_by(rect, large_edges);
        assert_eq!(clamped_inset.width(), 0.0);
        assert_eq!(clamped_inset.height(), 0.0);
    }

    #[test]
    fn test_dimensions_area_rect() {
        let dims = Dimensions::from_content_box(
            Rect::new(10.0, 20.0, 100.0, 50.0),
            EdgeSizes::all(5.0),
            EdgeSizes::all(2.0),
            EdgeSizes::all(10.0),
        );

        assert_eq!(dims.area_rect(crate::box_model::BoxArea::Content), dims.content_box());
        assert_eq!(dims.area_rect(crate::box_model::BoxArea::Padding), dims.padding_box());
        assert_eq!(dims.area_rect(crate::box_model::BoxArea::Border), dims.border_box());
        assert_eq!(dims.area_rect(crate::box_model::BoxArea::Margin), dims.margin_box());
    }
}

