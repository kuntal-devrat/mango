use crate::box_tree::LayoutBox;

/// CSS 2.1 Appendix E paint layers for ordering in-flow and positioned content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PaintLayer {
    /// Positioned children with negative z-index (CSS 2.1 App E layer 2)
    NegativeZ(i32),
    /// Block-level normal flow backgrounds (CSS 2.1 App E layer 3)
    BlockBackgrounds,
    /// Non-positioned floats (CSS 2.1 App E layer 4)
    Float,
    /// In-flow inline content (CSS 2.1 App E layer 5)
    Inline,
    /// Positioned children with z-index: auto or z-index: 0 (CSS 2.1 App E layer 6)
    ZeroOrAutoZ,
    /// Positioned children with positive z-index (CSS 2.1 App E layer 7)
    PositiveZ(i32),
    /// Top-layer elements (modal dialogs and open popovers) rendered above all normal z-index stacking contexts
    TopLayer(u32),
}

pub(crate) fn paint_layer_of(box_node: &LayoutBox) -> PaintLayer {
    let is_top_layer = (box_node.tag_name.as_deref() == Some("dialog")
        && box_node.get_attribute("open").is_some())
        || box_node.get_attribute("data-mango-popover-open") == Some("true")
        || box_node.get_attribute(":popover-open") == Some("true");
    if is_top_layer {
        return PaintLayer::TopLayer(1);
    }
    if let Some(s) = &box_node.style {
        let creates_stacking_context = s.position != mango_css::values::Position::Static
            || s.opacity < 1.0
            || !s.transform.is_identity()
            || !s.filter.is_empty()
            || !s.backdrop_filter.is_empty()
            || s.isolation == mango_css::values::Isolation::Isolate
            || s.mix_blend_mode != mango_css::values::BlendMode::Normal
            || s.clip_path != mango_css::values::ClipPath::None
            || s.mask_image.is_some()
            || matches!(
                s.contain,
                mango_css::values::Contain::Paint | mango_css::values::Contain::Strict
            )
            || s.will_change;

        if creates_stacking_context {
            match s.z_index {
                Some(z) if z < 0 => PaintLayer::NegativeZ(z),
                Some(z) if z > 0 => PaintLayer::PositiveZ(z),
                _ => PaintLayer::ZeroOrAutoZ,
            }
        } else if s.float != mango_css::values::Float::None {
            PaintLayer::Float
        } else if box_node.is_inline() {
            PaintLayer::Inline
        } else if has_positioned_descendant(box_node) {
            PaintLayer::ZeroOrAutoZ
        } else {
            PaintLayer::BlockBackgrounds
        }
    } else if box_node.is_inline() {
        PaintLayer::Inline
    } else if has_positioned_descendant(box_node) {
        PaintLayer::ZeroOrAutoZ
    } else {
        PaintLayer::BlockBackgrounds
    }
}

fn has_positioned_descendant(b: &LayoutBox) -> bool {
    // Memoized per box: assigning paint layers previously rescanned each
    // child's subtree, which is quadratic on deep documents.
    b.has_positioned_descendant()
}
