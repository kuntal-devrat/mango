//! Type conversions between `mango_core` / `mango_css` and `webrender_api`.

use mango_core::{Color, Rect};
use webrender_api::units::*;
use webrender_api::*;

use crate::display_list::BorderWidths;

/// Converts a [`Color`] into a WebRender [`ColorF`].
#[inline]
pub fn to_colorf(color: Color) -> ColorF {
    ColorF::new(
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        color.a as f32 / 255.0,
    )
}

/// Converts an (x, y) coordinate into a [`LayoutPoint`].
#[inline]
pub fn to_layout_point(x: f32, y: f32) -> LayoutPoint {
    LayoutPoint::new(x, y)
}

/// Converts a (width, height) size into a [`LayoutSize`].
#[inline]
pub fn to_layout_size(w: f32, h: f32) -> LayoutSize {
    LayoutSize::new(w.max(0.0), h.max(0.0))
}

/// Converts a [`Rect`] into a WebRender [`LayoutRect`].
#[inline]
pub fn to_layout_rect(rect: &Rect) -> LayoutRect {
    LayoutRect::from_origin_and_size(
        to_layout_point(rect.x(), rect.y()),
        to_layout_size(rect.width(), rect.height()),
    )
}

/// Converts corner radii `[top-left, top-right, bottom-right, bottom-left]` into [`BorderRadius`].
#[inline]
pub fn to_border_radius(radii: [f32; 4]) -> BorderRadius {
    BorderRadius {
        top_left: LayoutSize::new(radii[0].max(0.0), radii[0].max(0.0)),
        top_right: LayoutSize::new(radii[1].max(0.0), radii[1].max(0.0)),
        bottom_right: LayoutSize::new(radii[2].max(0.0), radii[2].max(0.0)),
        bottom_left: LayoutSize::new(radii[3].max(0.0), radii[3].max(0.0)),
    }
}

/// Converts [`BorderWidths`] into WebRender [`LayoutSideOffsets`].
#[inline]
pub fn to_border_widths(widths: &BorderWidths) -> LayoutSideOffsets {
    LayoutSideOffsets::new(
        widths.top.max(0.0),
        widths.right.max(0.0),
        widths.bottom.max(0.0),
        widths.left.max(0.0),
    )
}

/// Converts a 2D affine transform matrix `[a, b, c, d, e, f]` into a 4x4 [`LayoutTransform`].
#[inline]
pub fn to_layout_transform(matrix: [f32; 6]) -> LayoutTransform {
    LayoutTransform::new(
        matrix[0], matrix[1], 0.0, 0.0, matrix[2], matrix[3], 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
        matrix[4], matrix[5], 0.0, 1.0,
    )
}
