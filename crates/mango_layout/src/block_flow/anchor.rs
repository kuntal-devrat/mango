use mango_css::values::{AnchorSide, Length};

use crate::box_tree::LayoutBox;

thread_local! {
    static ANCHOR_REGISTRY: std::cell::RefCell<std::collections::HashMap<String, mango_core::Rect>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Registers an element's border box under its anchor-name ident.
pub fn register_anchor(name: &str, rect: mango_core::Rect) {
    ANCHOR_REGISTRY.with(|reg| {
        reg.borrow_mut().insert(name.to_string(), rect);
    });
}

/// Looks up an anchor box rectangle by name.
pub fn get_anchor(name: &str) -> Option<mango_core::Rect> {
    ANCHOR_REGISTRY.with(|reg| reg.borrow().get(name).copied())
}

/// Clears all registered anchors in the current thread.
pub fn clear_anchors() {
    ANCHOR_REGISTRY.with(|reg| reg.borrow_mut().clear());
}

/// Recursively registers any box with `anchor_name` in the subtree.
pub fn collect_anchors(box_node: &LayoutBox) {
    if let Some(style) = &box_node.style {
        if let Some(ref name) = style.anchor_name {
            register_anchor(name, box_node.dimensions.border_box());
        }
    }
    for child in &box_node.children {
        collect_anchors(child);
    }
}

/// Resolves an anchor() length reference into an absolute document coordinate.
pub fn resolve_anchor_coord(
    len: &Length,
    is_x: bool,
    default_anchor: Option<&String>,
) -> Option<f32> {
    if let Length::Anchor {
        name_id,
        side,
        offset,
    } = len
    {
        let name = name_id
            .and_then(mango_css::values::get_interned_anchor_name)
            .or_else(|| default_anchor.cloned())?;
        let rect = get_anchor(&name)?;
        let coord = match side {
            AnchorSide::Left | AnchorSide::Start => rect.left(),
            AnchorSide::Right | AnchorSide::End => rect.right(),
            AnchorSide::Center => {
                if is_x {
                    rect.origin.x + rect.width() * 0.5
                } else {
                    rect.origin.y + rect.height() * 0.5
                }
            }
            AnchorSide::Top => rect.top(),
            AnchorSide::Bottom => rect.bottom(),
        };
        Some(coord + offset)
    } else {
        None
    }
}
