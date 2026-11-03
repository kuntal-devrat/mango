use mango_core::Rect;
use mango_css::parser::parse_stylesheet;
use mango_html::parse_html;

use crate::box_tree::{LayoutBox, build_box_tree};
use crate::dimensions::Dimensions;
use crate::float::FloatContext;
use crate::style_tree::build_style_tree;

use super::compute_taffy_flex_or_grid;

fn layout_with_taffy(html: &str, css: &str, cb_w: f32, cb_h: f32) -> LayoutBox {
    let doc = parse_html(html);
    let sheet = parse_stylesheet(css);
    let style_tree = build_style_tree(&doc, &[&sheet]).expect("failed to build style tree");
    let mut box_tree = build_box_tree(&style_tree);

    let cb = Dimensions::new(Rect::new(0.0, 0.0, cb_w, cb_h));
    let mut float_ctx = FloatContext::new();

    fn find_flex_or_grid_mut(node: &mut LayoutBox) -> Option<&mut LayoutBox> {
        if let Some(s) = &node.style
            && matches!(
                s.display,
                mango_css::values::Display::Flex
                    | mango_css::values::Display::InlineFlex
                    | mango_css::values::Display::Grid
                    | mango_css::values::Display::InlineGrid
            )
        {
            return Some(node);
        }
        for child in &mut node.children {
            if let Some(f) = find_flex_or_grid_mut(child) {
                return Some(f);
            }
        }
        None
    }

    let container = find_flex_or_grid_mut(&mut box_tree).expect("no flex or grid container found");
    let handled = compute_taffy_flex_or_grid(container, &cb, &mut float_ctx);
    assert!(handled, "Taffy layout failed to execute");
    container.clone()
}

#[test]
fn test_taffy_flex_row_distribution() {
    let html = r#"<div class="container"><div class="item">A</div><div class="item">B</div></div>"#;
    let css = r#"
        .container {
            display: flex;
            flex-direction: row;
            width: 400px;
            height: 100px;
            justify-content: space-between;
        }
        .item {
            width: 100px;
            height: 50px;
        }
    "#;

    let container = layout_with_taffy(html, css, 800.0, 600.0);
    assert_eq!(container.dimensions.content.size.width, 400.0);
    assert_eq!(container.dimensions.content.size.height, 100.0);

    // Item 1 placed at x = 0
    assert_eq!(container.children[0].dimensions.content.origin.x, 0.0);
    assert_eq!(container.children[0].dimensions.content.size.width, 100.0);

    // Item 2 placed at x = 300 with space-between (400 - 100 = 300)
    assert_eq!(container.children[1].dimensions.content.origin.x, 300.0);
    assert_eq!(container.children[1].dimensions.content.size.width, 100.0);
}

#[test]
fn test_taffy_flex_grow_and_gap() {
    let html = r#"<div class="container"><div class="item">A</div><div class="item">B</div></div>"#;
    let css = r#"
        .container {
            display: flex;
            flex-direction: row;
            width: 320px;
            height: 100px;
            column-gap: 20px;
        }
        .item {
            flex-grow: 1;
            flex-basis: 0px;
        }
    "#;

    let container = layout_with_taffy(html, css, 500.0, 500.0);
    // Remaining space: 320 - 20 (gap) = 300. Equal grow: 150 each.
    assert_eq!(container.children[0].dimensions.content.size.width, 150.0);
    assert_eq!(container.children[1].dimensions.content.size.width, 150.0);
    assert_eq!(container.children[1].dimensions.content.origin.x, 170.0);
}

#[test]
fn test_taffy_grid_two_columns() {
    let html = r#"<div class="container"><div class="cell">1</div><div class="cell">2</div><div class="cell">3</div><div class="cell">4</div></div>"#;
    let css = r#"
        .container {
            display: grid;
            width: 400px;
            height: 200px;
            grid-template-columns: 150px 250px;
            grid-template-rows: 100px 100px;
        }
    "#;

    let grid = layout_with_taffy(html, css, 800.0, 600.0);
    assert_eq!(grid.dimensions.content.size.width, 400.0);
    assert_eq!(grid.dimensions.content.size.height, 200.0);

    // Cell 1: row 1, col 1 -> (0, 0), size 150x100
    assert_eq!(grid.children[0].dimensions.content.origin.x, 0.0);
    assert_eq!(grid.children[0].dimensions.content.origin.y, 0.0);
    assert_eq!(grid.children[0].dimensions.content.size.width, 150.0);

    // Cell 2: row 1, col 2 -> (150, 0), size 250x100
    assert_eq!(grid.children[1].dimensions.content.origin.x, 150.0);
    assert_eq!(grid.children[1].dimensions.content.origin.y, 0.0);
    assert_eq!(grid.children[1].dimensions.content.size.width, 250.0);

    // Cell 3: row 2, col 1 -> (0, 100), size 150x100
    assert_eq!(grid.children[2].dimensions.content.origin.x, 0.0);
    assert_eq!(grid.children[2].dimensions.content.origin.y, 100.0);

    // Cell 4: row 2, col 2 -> (150, 100), size 250x100
    assert_eq!(grid.children[3].dimensions.content.origin.x, 150.0);
    assert_eq!(grid.children[3].dimensions.content.origin.y, 100.0);
}

#[test]
fn test_taffy_grid_fr_units_and_gap() {
    let html = r#"<div class="container"><div class="col">A</div><div class="col">B</div></div>"#;
    let css = r#"
        .container {
            display: grid;
            width: 500px;
            height: 100px;
            grid-template-columns: 1fr 2fr;
            column-gap: 20px;
        }
    "#;

    let grid = layout_with_taffy(html, css, 800.0, 600.0);
    // 500 - 20 = 480. 1fr = 160, 2fr = 320.
    assert_eq!(grid.children[0].dimensions.content.size.width, 160.0);
    assert_eq!(grid.children[1].dimensions.content.size.width, 320.0);
    assert_eq!(grid.children[1].dimensions.content.origin.x, 180.0);
}
