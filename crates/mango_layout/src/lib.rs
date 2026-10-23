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
    clippy::manual_memcpy,
    clippy::type_complexity,
    clippy::collapsible_if
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

pub use a11y::{A11yNode, A11yRole, A11yState, A11yTree, CheckedState, PressedState};
pub use bidi::{BidiRun, bidi_visual_runs};
pub use block_flow::layout_block;
pub use box_model::{
    BoxArea, BoxType, Direction, IFrameSandbox, LogicalEdgeSizes, LogicalRect, MarginStrut,
    OverflowModel, WritingMode, adjust_for_box_sizing, resolve_object_fit_rect,
    resolve_replaced_size,
};
pub use box_tree::{
    FormControlHit, LayoutBox, MediaClickAction, MediaControlHit, build_box_tree,
    build_box_tree_with_maps,
};
pub use dimensions::Dimensions;
pub use display_list::{
    COLOR_SWATCHES, DisplayListCache, build_display_list, build_display_list_with_scroll,
    build_display_list_with_scroll_xy,
};
pub use flex_flow::layout_flex;
pub use float::FloatContext;
pub use grid_flow::{layout_grid, measure_grid_intrinsic_widths};
pub use inline_flow::{layout_inline_children, measure_text_width};
pub use mango_render::{DiffOp, DisplayListDiff};
pub use shaping::{
    is_arabic_char, is_complex_char, is_devanagari_char, is_thai_char, segment_thai_syllables,
    shape_arabic, shape_complex_script, shape_devanagari, shape_thai,
};
pub use snapshot::LayoutSnapshot;
pub use style_tree::{
    PageStyleIndex, StyleInvalidator, StyledNode, build_style_tree,
    build_style_tree_with_cached_index, build_style_tree_with_env, build_style_tree_with_size,
    collect_image_maps, extract_style_elements,
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
    if let Some(style_arc) = root.style.as_mut() {
        let style = std::sync::Arc::make_mut(style_arc);
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
    relayout_box_tree_with_scroll_xy(root, viewport, 0.0, 0.0)
}

/// Re-runs layout for an already-built box tree and regenerates its display list with
/// custom horizontal and vertical scroll offsets.
pub fn relayout_box_tree_with_scroll_xy(
    root: &mut LayoutBox,
    viewport: Size,
    scroll_x: f32,
    scroll_y: f32,
) -> DisplayList {
    mango_css::set_current_viewport(viewport.width, viewport.height);
    let containing_block = Dimensions::new(Rect::new(0.0, 0.0, viewport.width, viewport.height));
    let mut float_ctx = FloatContext::new();
    layout_block(root, &containing_block, &mut float_ctx);

    // Explicitly reset children before the traversal so that a re-layout of an
    // already laid-out tree does not accumulate stale offsets.
    build_display_list_with_scroll_xy(root, scroll_x, scroll_y)
}

/// Collects `(DOM node id, computed style)` pairs for every styled box in the tree.
pub fn collect_box_styles(
    root: &LayoutBox,
    out: &mut std::collections::HashMap<u32, mango_css::computed::ComputedStyle>,
) {
    if let (Some(node_id), Some(style)) = (root.node_id, root.style.as_ref()) {
        out.insert(node_id.raw(), (**style).clone());
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
    layout_document_with_scroll_xy(doc, author_styles, viewport, 0.0, 0.0)
}

/// Fully lays out an HTML DOM [`Document`] with an explicit [`MediaEnvironment`].
pub fn layout_document_with_env(
    doc: &Document,
    author_styles: &[&Stylesheet],
    env: &mango_css::MediaEnvironment,
    scroll_x: f32,
    scroll_y: f32,
) -> (LayoutBox, DisplayList) {
    mango_css::set_current_viewport(env.viewport_width, env.viewport_height);
    let Some(styled_root) = build_style_tree_with_env(doc, author_styles, env) else {
        return (LayoutBox::new(BoxType::BlockNode, None), DisplayList::new());
    };

    layout_styled_document(doc, styled_root, env, scroll_x, scroll_y)
}

/// Lays out a document using a caller-owned page stylesheet index. The index
/// is not rebuilt during this operation, which is the hot path for relayouts.
pub fn layout_document_with_style_index(
    doc: &Document,
    style_index: &PageStyleIndex,
    viewport: Size,
) -> (LayoutBox, DisplayList) {
    let mut env = mango_css::current_media_environment();
    env.viewport_width = viewport.width;
    env.viewport_height = viewport.height;
    let Some(styled_root) = build_style_tree_with_cached_index(doc, style_index.index(), &env)
    else {
        return (LayoutBox::new(BoxType::BlockNode, None), DisplayList::new());
    };
    layout_styled_document(doc, styled_root, &env, 0.0, 0.0)
}

fn layout_styled_document(
    doc: &Document,
    styled_root: StyledNode,
    env: &mango_css::MediaEnvironment,
    scroll_x: f32,
    scroll_y: f32,
) -> (LayoutBox, DisplayList) {
    let maps = collect_image_maps(doc);
    let mut root_box = build_box_tree_with_maps(&styled_root, Some(&maps));

    let containing_block =
        Dimensions::new(Rect::new(0.0, 0.0, env.viewport_width, env.viewport_height));
    let mut float_ctx = FloatContext::new();

    layout_block(&mut root_box, &containing_block, &mut float_ctx);

    clamp_scroll_offsets(&mut root_box);

    let display_list = build_display_list_with_scroll_xy(&root_box, scroll_x, scroll_y);

    (root_box, display_list)
}

fn clamp_scroll_offsets(box_node: &mut LayoutBox) {
    if box_node.is_scroll_container() {
        let (max_x, max_y) = box_node.max_scroll();
        box_node.scroll_offset_x = box_node.scroll_offset_x.clamp(0.0, max_x);
        box_node.scroll_offset_y = box_node.scroll_offset_y.clamp(0.0, max_y);
    }
    for child in &mut box_node.children {
        clamp_scroll_offsets(child);
    }
}

/// Fully lays out an HTML DOM [`Document`], applying author stylesheets and inline styles,
/// with explicit 2D horizontal and vertical scroll offsets applied to the resulting display list.
pub fn layout_document_with_scroll_xy(
    doc: &Document,
    author_styles: &[&Stylesheet],
    viewport: Size,
    scroll_x: f32,
    scroll_y: f32,
) -> (LayoutBox, DisplayList) {
    let mut env = mango_css::current_media_environment();
    env.viewport_width = viewport.width;
    env.viewport_height = viewport.height;
    layout_document_with_env(doc, author_styles, &env, scroll_x, scroll_y)
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

    #[test]
    fn test_layout_document_with_scroll_xy() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <body style="margin: 0;">
                    <div style="width: 200px; height: 100px; background-color: #ff0000;">Box</div>
                </body>
            </html>
        "#;
        let doc = parse_html(html);
        let viewport = Size::new(800.0, 600.0);

        let (mut root, dl_scroll) = layout_document_with_scroll_xy(&doc, &[], viewport, 20.0, 40.0);
        assert!(!dl_scroll.is_empty());

        let dl_relayout = relayout_box_tree_with_scroll_xy(&mut root, viewport, 20.0, 40.0);
        assert!(!dl_relayout.is_empty());
    }

    #[test]
    fn test_cached_style_index_handles_variable_heavy_page() {
        let started = std::time::Instant::now();
        let variables = (0..1200)
            .map(|i| format!("--github-token-{i}: {i}px;"))
            .collect::<String>();
        let css = mango_css::parse_stylesheet(&format!(
            ":root {{ {variables} }} .item {{ color: #333; padding: 2px; }}"
        ));
        let body = (0..1200)
            .map(|i| format!("<div class=\"item\">item {i}</div>"))
            .collect::<String>();
        let doc = parse_html(&format!("<html><body>{body}</body></html>"));
        let index =
            PageStyleIndex::from_stylesheets(vec![css], mango_css::current_media_environment());

        // Repeated layout is the stress case: the stylesheet/index must be
        // reusable and the test must complete without rebuilding the rule
        // index or cloning the inherited variable map per node.
        for _ in 0..4 {
            let (root, display_list) =
                layout_document_with_style_index(&doc, &index, Size::new(800.0, 600.0));
            assert!(root.descendant_count() >= 1200);
            assert!(!display_list.is_empty());
        }

        // Keep this generous enough for debug CI, while catching accidental
        // O(stylesheets × DOM nodes) regressions and pathological CPU churn.
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        #[cfg(target_os = "linux")]
        if let Ok(status) = std::fs::read_to_string("/proc/self/status")
            && let Some(rss_kib) = status.lines().find_map(|line| {
                line.strip_prefix("VmRSS:")
                    .and_then(|v| v.split_whitespace().next())
                    .and_then(|v| v.parse::<u64>().ok())
            })
        {
            assert!(rss_kib < 768 * 1024, "stress layout RSS was {rss_kib} KiB");
        }
    }
}
