use mango_core::geometry::{EdgeSizes, Rect, Size};
use mango_css::values::{BoxSizing, ObjectFit};

/// Resolves the used width and height of a replaced element conforming to
/// CSS 2.1 §10.3.2, §10.6.2 and CSS Sizing Module Level 3 §5.2.
pub fn resolve_replaced_size(
    specified_width: Option<f32>,
    specified_height: Option<f32>,
    intrinsic_width: Option<f32>,
    intrinsic_height: Option<f32>,
    aspect_ratio: Option<f32>,
    default_fallback: (f32, f32),
) -> (f32, f32) {
    let ratio = aspect_ratio.or_else(|| match (intrinsic_width, intrinsic_height) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some(w / h),
        _ => None,
    });

    match (specified_width, specified_height) {
        // 1. Both dimensions specified explicitly.
        (Some(w), Some(h)) => (w.max(0.0), h.max(0.0)),

        // 2. Width specified, height auto.
        (Some(w), None) => {
            let used_w = w.max(0.0);
            let used_h = if let Some(r) = ratio.filter(|&r| r > 0.0) {
                (used_w / r).max(0.0)
            } else if let Some(ih) = intrinsic_height {
                ih.max(0.0)
            } else {
                default_fallback.1
            };
            (used_w, used_h)
        }

        // 3. Height specified, width auto.
        (None, Some(h)) => {
            let used_h = h.max(0.0);
            let used_w = if let Some(r) = ratio.filter(|&r| r > 0.0) {
                (used_h * r).max(0.0)
            } else if let Some(iw) = intrinsic_width {
                iw.max(0.0)
            } else {
                default_fallback.0
            };
            (used_w, used_h)
        }

        // 4. Both dimensions auto.
        (None, None) => match (intrinsic_width, intrinsic_height) {
            (Some(iw), Some(ih)) => (iw.max(0.0), ih.max(0.0)),
            (Some(iw), None) => {
                let used_w = iw.max(0.0);
                let used_h = if let Some(r) = ratio.filter(|&r| r > 0.0) {
                    (used_w / r).max(0.0)
                } else {
                    default_fallback.1
                };
                (used_w, used_h)
            }
            (None, Some(ih)) => {
                let used_h = ih.max(0.0);
                let used_w = if let Some(r) = ratio.filter(|&r| r > 0.0) {
                    (used_h * r).max(0.0)
                } else {
                    default_fallback.0
                };
                (used_w, used_h)
            }
            (None, None) => {
                if let Some(r) = ratio.filter(|&r| r > 0.0) {
                    let (dw, dh) = default_fallback;
                    if dw / dh > r {
                        ((dh * r).max(0.0), dh)
                    } else {
                        (dw, (dw / r).max(0.0))
                    }
                } else {
                    default_fallback
                }
            }
        },
    }
}

/// Computes the destination rectangle of replaced content within its content box
/// according to `object-fit` and `object-position` (CSS Images Module Level 3 §5.5).
pub fn resolve_object_fit_rect(
    content_box: Rect,
    natural_size: (f32, f32),
    object_fit: ObjectFit,
    object_position: (f32, f32),
) -> Rect {
    let (box_w, box_h) = (content_box.width(), content_box.height());
    let (nat_w, nat_h) = natural_size;

    if box_w <= 0.0 || box_h <= 0.0 || nat_w <= 0.0 || nat_h <= 0.0 {
        return content_box;
    }

    let (draw_w, draw_h) = match object_fit {
        ObjectFit::Fill => (box_w, box_h),
        ObjectFit::Contain => {
            let scale = (box_w / nat_w).min(box_h / nat_h);
            (nat_w * scale, nat_h * scale)
        }
        ObjectFit::Cover => {
            let scale = (box_w / nat_w).max(box_h / nat_h);
            (nat_w * scale, nat_h * scale)
        }
        ObjectFit::None => (nat_w, nat_h),
        ObjectFit::ScaleDown => {
            let scale_contain = (box_w / nat_w).min(box_h / nat_h);
            if scale_contain < 1.0 {
                (nat_w * scale_contain, nat_h * scale_contain)
            } else {
                (nat_w, nat_h)
            }
        }
    };

    // Calculate position: object_position is (percent_x, percent_y), defaulting to (0.5, 0.5)
    let offset_x = content_box.x() + (box_w - draw_w) * object_position.0;
    let offset_y = content_box.y() + (box_h - draw_h) * object_position.1;

    Rect::new(offset_x, offset_y, draw_w, draw_h)
}

/// Converts a specified size to content-box size according to `box-sizing` (CSS Box Sizing 3).
#[inline]
pub fn adjust_for_box_sizing(
    size: Size,
    box_sizing: BoxSizing,
    padding: &EdgeSizes,
    border: &EdgeSizes,
) -> Size {
    match box_sizing {
        BoxSizing::ContentBox => size,
        BoxSizing::BorderBox => Size::new(
            (size.width - padding.horizontal() - border.horizontal()).max(0.0),
            (size.height - padding.vertical() - border.vertical()).max(0.0),
        ),
    }
}
