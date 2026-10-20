use mango_css::values::{Display, Float, Overflow, Position};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

/// Determines if a layout box establishes an independent Block Formatting Context (BFC)
/// according to CSS 2.1 §9.4.1 and CSS Display 3.
pub fn establishes_bfc(box_node: &LayoutBox) -> bool {
    if box_node.is_independent_formatting_context {
        return true;
    }
    if matches!(box_node.box_type, BoxType::InlineBlock) {
        return true;
    }
    let Some(style) = &box_node.style else {
        return false;
    };
    if style.float != Float::None {
        return true;
    }
    if matches!(style.position, Position::Absolute | Position::Fixed) {
        return true;
    }
    if matches!(
        style.display,
        Display::FlowRoot | Display::InlineBlock | Display::TableCell | Display::TableCaption
    ) {
        return true;
    }
    // CSS Overflow 3 § 3.3: body overflow is propagated to viewport and does not establish a BFC
    let is_body = box_node.tag_name.as_deref() == Some("body");
    if !is_body
        && (matches!(
            style.overflow_x,
            Overflow::Hidden | Overflow::Scroll | Overflow::Auto
        ) || matches!(
            style.overflow_y,
            Overflow::Hidden | Overflow::Scroll | Overflow::Auto
        ))
    {
        return true;
    }
    if style.column_count.is_some() || style.column_width.is_some() {
        return true;
    }
    false
}
