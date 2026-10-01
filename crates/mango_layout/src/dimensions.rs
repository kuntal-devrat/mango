//! Box dimensions and geometric representations for the CSS box model.

use mango_core::{EdgeSizes, Rect};

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

    pub fn new(content: Rect) -> Self {
        Self {
            content,
            padding: EdgeSizes::ZERO,
            border: EdgeSizes::ZERO,
            margin: EdgeSizes::ZERO,
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
            self.content.width() + self.padding.horizontal(),
            self.content.height() + self.padding.vertical(),
        )
    }

    /// The border box (padding box + border).
    #[inline]
    pub fn border_box(&self) -> Rect {
        let pad = self.padding_box();
        Rect::new(
            pad.x() - self.border.left,
            pad.y() - self.border.top,
            pad.width() + self.border.horizontal(),
            pad.height() + self.border.vertical(),
        )
    }

    /// The margin box (border box + margin).
    #[inline]
    pub fn margin_box(&self) -> Rect {
        let b = self.border_box();
        Rect::new(
            b.x() - self.margin.left,
            b.y() - self.margin.top,
            b.width() + self.margin.horizontal(),
            b.height() + self.margin.vertical(),
        )
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
    }
}
