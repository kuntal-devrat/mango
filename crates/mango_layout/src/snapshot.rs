//! Immutable layout snapshots (ARCH-003).
//!
//! Enforces type-level immutability for the layout tree:
//! - Layout consumes an immutable borrow `&StyledNode` (the styled DOM tree).
//! - Produces an owned, frozen [`LayoutSnapshot`].
//! - No mutation is possible through the snapshot, eliminating "layout thrashing"
//!   and concurrent read/write coupling between script and layout.

use std::collections::HashMap;

use mango_core::{Point, Rect, Size};
use mango_css::computed::ComputedStyle;
use mango_html::dom::{Document, NodeId};
use mango_render::display_list::DisplayList;

use crate::block_flow::layout_block;
use crate::box_tree::{FormControlHit, LayoutBox, build_box_tree};
use crate::dimensions::Dimensions;
use crate::display_list::{build_display_list, build_display_list_with_scroll};
use crate::float::FloatContext;
use crate::style_tree::{StyledNode, build_style_tree_with_size};

/// A frozen, read-only layout tree snapshot for a given viewport (ARCH-003).
///
/// Immutability guarantees that layout results cannot be mutated in place,
/// preventing the entire class of "layout thrashing" bugs.
#[derive(Debug, Clone)]
pub struct LayoutSnapshot {
    root_box: LayoutBox,
    viewport: Size,
    display_list: DisplayList,
    box_rects: HashMap<u32, [f32; 4]>,
}

impl LayoutSnapshot {
    /// Creates a new immutable layout snapshot from an immutable reference to the styled tree.
    ///
    /// Consumes `&StyledNode` without mutating it, performs layout within the given viewport,
    /// precomputes the display list and bounding rectangles, and freezes the tree.
    pub fn from_styled_tree(styled_root: &StyledNode, viewport: Size) -> Self {
        let mut root_box = build_box_tree(styled_root);
        let containing_block =
            Dimensions::new(Rect::new(0.0, 0.0, viewport.width, viewport.height));
        let mut float_ctx = FloatContext::new();

        layout_block(&mut root_box, &containing_block, &mut float_ctx);

        let display_list = build_display_list(&root_box);

        let mut box_rects = HashMap::new();
        crate::collect_box_rects(&root_box, &mut box_rects);

        Self {
            root_box,
            viewport,
            display_list,
            box_rects,
        }
    }

    /// Convenience constructor to lay out a DOM [`Document`] directly into an immutable snapshot.
    pub fn from_document(
        doc: &Document,
        author_styles: &[&mango_css::parser::Stylesheet],
        viewport: Size,
    ) -> Option<Self> {
        let styled_root =
            build_style_tree_with_size(doc, author_styles, viewport.width, viewport.height)?;
        Some(Self::from_styled_tree(&styled_root, viewport))
    }

    /// Returns an immutable reference to the root layout box.
    pub fn root_box(&self) -> &LayoutBox {
        &self.root_box
    }

    /// Returns the viewport dimensions used when generating this snapshot.
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// Returns the precomputed display list for this snapshot.
    pub fn display_list(&self) -> &DisplayList {
        &self.display_list
    }

    /// Generates a display list with a custom vertical scroll offset.
    pub fn display_list_with_scroll(&self, scroll_y: f32) -> DisplayList {
        build_display_list_with_scroll(&self.root_box, scroll_y)
    }

    /// Returns the total content dimensions computed during layout.
    pub fn content_size(&self) -> Size {
        Size::new(
            self.root_box.dimensions.content.width(),
            self.root_box.dimensions.content.height(),
        )
    }

    /// Returns the precomputed border-box rectangles for all DOM-backed layout boxes.
    pub fn box_rects(&self) -> &HashMap<u32, [f32; 4]> {
        &self.box_rects
    }

    /// Looks up the border-box bounds of a specific DOM node.
    pub fn get_node_rect(&self, node_id: NodeId) -> Option<Rect> {
        self.box_rects
            .get(&node_id.raw())
            .map(|&[x, y, w, h]| Rect::new(x, y, w, h))
    }

    /// Performs hit-testing against the frozen layout tree, returning the deepest hit DOM node.
    pub fn hit_test(&self, point: Point) -> Option<NodeId> {
        fn hit_test_box(b: &LayoutBox, p: Point) -> Option<NodeId> {
            let rect = b.dimensions.border_box();
            if !rect.contains(p) {
                return None;
            }
            for child in b.children.iter().rev() {
                if let Some(hit) = hit_test_box(child, p) {
                    return Some(hit);
                }
            }
            b.node_id
        }
        hit_test_box(&self.root_box, point)
    }

    /// Performs hit-testing against hyperlinks in the layout tree.
    pub fn hit_test_link(&self, point: Point) -> Option<&str> {
        self.root_box.hit_test_link(point)
    }

    /// Performs hit-testing against interactive form controls in the layout tree.
    pub fn hit_test_form_control(&self, point: Point) -> Option<FormControlHit> {
        self.root_box.hit_test_form_control(point)
    }

    /// Collects computed styles for all DOM-backed boxes in the tree.
    pub fn collect_styles(&self) -> HashMap<u32, ComputedStyle> {
        let mut styles = HashMap::new();
        crate::collect_box_styles(&self.root_box, &mut styles);
        styles
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_html::parse_html;

    #[test]
    fn test_immutable_layout_snapshot_creation() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <style>
                        body { margin: 0; padding: 20px; }
                        h1 { color: #ff5500; font-size: 24px; }
                        p { color: #333333; font-size: 14px; }
                        .btn { width: 120px; height: 40px; background-color: #0088ff; }
                    </style>
                </head>
                <body>
                    <h1>Immutable Snapshot Test</h1>
                    <p>Testing ARCH-003 immutable layout snapshots</p>
                    <button class="btn">Click me</button>
                </body>
            </html>
        "#;
        let doc = parse_html(html);
        let viewport = Size::new(800.0, 600.0);

        let snapshot = LayoutSnapshot::from_document(&doc, &[], viewport)
            .expect("snapshot creation succeeded");

        assert_eq!(snapshot.viewport(), viewport);
        assert!(snapshot.content_size().width > 0.0);
        assert!(snapshot.content_size().height > 0.0);
        assert!(!snapshot.display_list().is_empty());

        // Snapshot is completely immutable — only provides read-only inspection methods
        assert!(!snapshot.box_rects().is_empty());
        let root = snapshot.root_box();
        assert_eq!(root.children.len(), 1); // <body>
    }

    #[test]
    fn test_snapshot_hit_test() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <body style="margin: 0;">
                    <div id="target" style="width: 200px; height: 100px; background-color: #eee;">
                        Box
                    </div>
                </body>
            </html>
        "#;
        let doc = parse_html(html);
        let viewport = Size::new(800.0, 600.0);

        let snapshot = LayoutSnapshot::from_document(&doc, &[], viewport).unwrap();

        // Hit-test inside the box
        let hit = snapshot.hit_test(Point::new(50.0, 50.0));
        assert!(hit.is_some());
    }
}
