//! # mango_layout
//!
//! Layout engine for the Mango browser.
//!
//! Converts a styled DOM tree into positioned boxes adhering to the CSS 2.1
//! visual formatting model:
//! - **Dimensions & Box Model**: content box, padding box, border box, margin box.
//! - **Style Tree**: Computed styles attached to DOM nodes with `display: none` pruning.
//! - **Box Tree**: Structural box generation with anonymous block box grouping.
//! - **Block Formatting Context**: Width resolution, `margin: auto` centering, vertical margin collapsing, auto height.
//! - **Inline Formatting Context**: Word-wrapped line boxes with font-scaled text measurement and `text-align`.
//! - **Float Layout**: Float positioning and clearance context.
//! - **Display List**: Flattens positioned layout boxes into render commands.

#![allow(
    clippy::field_reassign_with_default,
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::large_enum_variant,
    clippy::if_same_then_else,
    clippy::manual_memcpy
)]

pub mod a11y;
pub mod bidi;
pub mod block_flow;
pub mod box_model;
pub mod box_tree;
pub mod dimensions;
pub mod display_list;
pub mod flex_flow;
pub mod float;
pub mod grid_flow;
pub mod inline_flow;
pub mod shaping;
pub mod snapshot;
pub mod style_tree;
pub mod table_flow;

pub use a11y::{A11yNode, A11yRole, A11yState, A11yTree};
pub use block_flow::layout_block;
pub use box_model::BoxType;
pub use box_tree::{FormControlHit, LayoutBox, MediaClickAction, MediaControlHit, build_box_tree};
pub use dimensions::Dimensions;
pub use display_list::{
    COLOR_SWATCHES, DisplayListCache, build_display_list, build_display_list_with_scroll,
};
pub use flex_flow::layout_flex;
pub use float::FloatContext;
pub use grid_flow::layout_grid;
pub use inline_flow::{layout_inline_children, measure_text_width};
pub use mango_render::{DiffOp, DisplayListDiff};
pub use snapshot::LayoutSnapshot;
pub use style_tree::{
    StyleInvalidator, StyledNode, build_style_tree, build_style_tree_with_size,
    extract_style_elements,
};
pub use table_flow::layout_table;

use mango_core::{Rect, Size};
use mango_css::parser::Stylesheet;
use mango_html::dom::Document;
use mango_render::display_list::DisplayList;

/// Visits every box in the layout tree that carries a computed style, allowing
/// callers to override styles in place (used by the animation/transition runtime).
pub fn apply_box_style_overrides<F>(root: &mut LayoutBox, f: &mut F)
where
    F: FnMut(Option<mango_html::dom::NodeId>, &mut mango_css::computed::ComputedStyle),
{
    if let Some(style) = root.style.as_mut() {
        f(root.node_id, style);
    }
    for child in &mut root.children {
        apply_box_style_overrides(child, f);
    }
}

/// Re-runs layout for an already-built box tree and regenerates its display list.
///
/// This is the fast path used for animation frames and nested-scroll updates: it
/// skips style recomputation and box-tree construction entirely.
pub fn relayout_box_tree(root: &mut LayoutBox, viewport: Size) -> DisplayList {
    mango_css::set_current_viewport(viewport.width, viewport.height);
    let containing_block = Dimensions::new(Rect::new(0.0, 0.0, viewport.width, viewport.height));
    let mut float_ctx = FloatContext::new();
    layout_block(root, &containing_block, &mut float_ctx);

    // Explicitly reset children before the traversal so that a re-layout of an
    // already laid-out tree does not accumulate stale offsets.
    build_display_list(root)
}

/// Collects `(DOM node id, computed style)` pairs for every styled box in the tree.
pub fn collect_box_styles(
    root: &LayoutBox,
    out: &mut std::collections::HashMap<u32, mango_css::computed::ComputedStyle>,
) {
    if let (Some(node_id), Some(style)) = (root.node_id, root.style.as_ref()) {
        out.insert(node_id.raw(), style.clone());
    }
    for child in &root.children {
        collect_box_styles(child, out);
    }
}

/// Collects the border-box geometry of every box backed by a DOM node.
///
/// Keyed by raw DOM node id so the JS runtime can answer `getBoundingClientRect()`,
/// `offsetWidth`, and `elementFromPoint()` without depending on this crate.
pub fn collect_box_rects(root: &LayoutBox, out: &mut std::collections::HashMap<u32, [f32; 4]>) {
    if let Some(node_id) = root.node_id {
        let border = root.dimensions.border_box();
        out.insert(
            node_id.raw(),
            [border.x(), border.y(), border.width(), border.height()],
        );
    }
    for child in &root.children {
        collect_box_rects(child, out);
    }
}

/// Fully lays out an HTML DOM [`Document`], applying author stylesheets and inline styles,
/// constrained by the given viewport size, and produces both the final [`LayoutBox`] tree
/// and paint-ready [`DisplayList`].
pub fn layout_document(
    doc: &Document,
    author_styles: &[&Stylesheet],
    viewport: Size,
) -> (LayoutBox, DisplayList) {
    mango_css::set_current_viewport(viewport.width, viewport.height);
    let Some(styled_root) =
        build_style_tree_with_size(doc, author_styles, viewport.width, viewport.height)
    else {
        return (LayoutBox::new(BoxType::BlockNode, None), DisplayList::new());
    };

    let mut root_box = build_box_tree(&styled_root);
    let containing_block = Dimensions::new(Rect::new(0.0, 0.0, viewport.width, viewport.height));
    let mut float_ctx = FloatContext::new();

    layout_block(&mut root_box, &containing_block, &mut float_ctx);
    let display_list = build_display_list(&root_box);

    (root_box, display_list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_html::parse_html;

    #[test]
    fn test_layout_document_end_to_end() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <title>Mango Layout Test</title>
                    <style>
                        body { margin: 10px; background-color: #ffffff; }
                        h1 { color: #ffa136; margin: 15px 0px; font-size: 32px; }
                        p { color: #333333; margin: 10px 0px; font-size: 16px; }
                        .card { background-color: #f0f0f0; padding: 12px; border-top-width: 2px; }
                    </style>
                </head>
                <body>
                    <h1>Welcome to Mango</h1>
                    <div class="card">
                        <p>Mango is ultra-lightweight and written in pure Rust.</p>
                    </div>
                </body>
            </html>
        "#;
        let doc = parse_html(html);
        let viewport = Size::new(800.0, 600.0);

        let (root_box, display_list) = layout_document(&doc, &[], viewport);

        // Root box should be laid out with positive dimensions
        assert!(root_box.dimensions.content.width() > 0.0);
        assert!(root_box.dimensions.content.height() > 0.0);

        // Display list should contain multiple drawing commands:
        // background fills, borders, texts
        assert!(!display_list.is_empty());
        assert!(display_list.len() >= 4);
    }
}
