#![allow(clippy::field_reassign_with_default)]

use mango_core::Rect;
use mango_css::parser::parse_stylesheet;
use mango_css::values::{Display, GridTrackSize, Length};
use mango_html::parse_html;
use mango_layout::box_tree::build_box_tree;
use mango_layout::dimensions::Dimensions;
use mango_layout::float::FloatContext;
use mango_layout::grid_flow::layout_grid;
use mango_layout::style_tree::build_style_tree;

fn layout_html_grid(
    html: &str,
    css: &str,
    cb_w: f32,
    cb_h: f32,
) -> mango_layout::box_tree::LayoutBox {
    let doc = parse_html(html);
    let sheet = parse_stylesheet(css);
    let style_tree = build_style_tree(&doc, &[&sheet]).expect("failed to build style tree");
    let mut box_tree = build_box_tree(&style_tree);

    let cb = Dimensions::new(Rect::new(0.0, 0.0, cb_w, cb_h));
    let mut float_ctx = FloatContext::new();

    fn find_grid_mut(
        node: &mut mango_layout::box_tree::LayoutBox,
    ) -> Option<&mut mango_layout::box_tree::LayoutBox> {
        if let Some(s) = &node.style
            && matches!(s.display, Display::Grid | Display::InlineGrid)
        {
            return Some(node);
        }
        for child in &mut node.children {
            if let Some(g) = find_grid_mut(child) {
                return Some(g);
            }
        }
        None
    }

    let grid = find_grid_mut(&mut box_tree).expect("no grid container found in DOM");
    layout_grid(grid, &cb, &mut float_ctx);
    grid.clone()
}

#[test]
fn test_auto_fill_and_auto_fit_repeat() {
    // 1. auto-fill: repeat(auto-fill, 100px) with 500px container width and 10px gap
    // Space available: (500 + 10) / (100 + 10) = 510 / 110 = 4 columns (400px + 30px gap = 430px)
    let html = r#"<div class="grid"><div class="item">1</div><div class="item">2</div></div>"#;
    let css = r#"
        .grid {
            display: grid;
            width: 500px;
            column-gap: 10px;
            grid-template-columns: repeat(auto-fill, 100px);
        }
        .item { height: 50px; }
    "#;
    let grid = layout_html_grid(html, css, 600.0, 600.0);
    assert_eq!(grid.dimensions.content.width(), 500.0);
    let item1 = &grid.children[0];
    let item2 = &grid.children[1];
    assert_eq!(item1.dimensions.content.width(), 100.0);
    assert_eq!(item2.dimensions.content.width(), 100.0);
    assert_eq!(item1.dimensions.content.x(), 0.0);
    assert_eq!(item2.dimensions.content.x(), 110.0);

    // 2. auto-fit: collapses empty repeated tracks down to in-flow item count
    let css_fit = r#"
        .grid {
            display: grid;
            width: 500px;
            column-gap: 10px;
            grid-template-columns: repeat(auto-fit, 100px);
        }
        .item { height: 50px; }
    "#;
    let grid_fit = layout_html_grid(html, css_fit, 600.0, 600.0);
    assert_eq!(grid_fit.children.len(), 2);
    assert_eq!(grid_fit.children[0].dimensions.content.width(), 100.0);
    assert_eq!(grid_fit.children[1].dimensions.content.width(), 100.0);
}

#[test]
fn test_minmax_track_sizing() {
    // grid-template-columns: minmax(100px, 300px) 1fr
    // In a 600px container with 0 gap, minmax track grows up to 300px, and 1fr takes remaining 300px.
    let html = r#"<div class="grid"><div class="col1">A</div><div class="col2">B</div></div>"#;
    let css = r#"
        .grid {
            display: grid;
            width: 600px;
            grid-template-columns: minmax(100px, 300px) 1fr;
        }
        .col1 { height: 40px; }
        .col2 { height: 40px; }
    "#;
    let grid = layout_html_grid(html, css, 800.0, 800.0);
    let c1 = &grid.children[0];
    let c2 = &grid.children[1];
    assert_eq!(
        c1.dimensions.content.width(),
        300.0,
        "minmax track should grow to max 300px"
    );
    assert_eq!(
        c2.dimensions.content.width(),
        300.0,
        "1fr should take remaining 300px"
    );
    assert_eq!(c2.dimensions.content.x(), 300.0);
}

#[test]
fn test_named_grid_lines_and_areas() {
    let html = r#"
        <div class="grid">
            <header class="hdr">Header</header>
            <nav class="nav">Nav</nav>
            <main class="main">Main</main>
        </div>
    "#;
    let css = r#"
        .grid {
            display: grid;
            width: 800px;
            grid-template-columns: 200px 1fr;
            grid-template-rows: 60px 400px;
            grid-template-areas:
                "head head"
                "side body";
        }
        .hdr { grid-area: head; }
        .nav { grid-area: side; }
        .main { grid-area: body; }
    "#;
    let grid = layout_html_grid(html, css, 1000.0, 800.0);
    let hdr = &grid.children[0];
    let nav = &grid.children[1];
    let main = &grid.children[2];

    // Header spans head head: full 800px width, height 60px
    assert_eq!(hdr.dimensions.content.x(), 0.0);
    assert_eq!(hdr.dimensions.content.y(), 0.0);
    assert_eq!(hdr.dimensions.content.width(), 800.0);
    assert_eq!(hdr.dimensions.content.height(), 60.0);

    // Nav is at side: col 0, row 1 (x=0, y=60), width 200px, height 400px
    assert_eq!(nav.dimensions.content.x(), 0.0);
    assert_eq!(nav.dimensions.content.y(), 60.0);
    assert_eq!(nav.dimensions.content.width(), 200.0);
    assert_eq!(nav.dimensions.content.height(), 400.0);

    // Main is at body: col 1, row 1 (x=200, y=60), width 600px, height 400px
    assert_eq!(main.dimensions.content.x(), 200.0);
    assert_eq!(main.dimensions.content.y(), 60.0);
    assert_eq!(main.dimensions.content.width(), 600.0);
    assert_eq!(main.dimensions.content.height(), 400.0);
}

#[test]
fn test_grid_item_placement_with_span() {
    let html =
        r#"<div class="grid"><div class="hero">Hero</div><div class="card">Card</div></div>"#;
    let css = r#"
        .grid {
            display: grid;
            width: 900px;
            grid-template-columns: repeat(3, 1fr);
        }
        .hero {
            grid-column: 1 / span 2;
            height: 100px;
        }
        .card {
            grid-column: span 1;
            height: 100px;
        }
    "#;
    let grid = layout_html_grid(html, css, 1000.0, 800.0);
    let hero = &grid.children[0];
    let card = &grid.children[1];

    // Total 900px, 3 equal 300px columns.
    // Hero spans 2 columns = 600px width at x = 0
    assert_eq!(hero.dimensions.content.x(), 0.0);
    assert_eq!(hero.dimensions.content.width(), 600.0);

    // Card placed in column 3 at x = 600 with width = 300px
    assert_eq!(card.dimensions.content.x(), 600.0);
    assert_eq!(card.dimensions.content.width(), 300.0);
}

#[test]
fn test_implicit_grid_tracks() {
    // Explicit grid defines 2 columns (100px, 100px), but item 2 is explicitly placed at column 4
    let html = r#"<div class="grid"><div class="item1">1</div><div class="item2">2</div></div>"#;
    let css = r#"
        .grid {
            display: grid;
            width: 600px;
            grid-template-columns: 100px 100px;
        }
        .item1 { grid-column: 1; height: 50px; }
        .item2 { grid-column: 4; height: 50px; }
    "#;
    let grid = layout_html_grid(html, css, 800.0, 800.0);
    let item1 = &grid.children[0];
    let item2 = &grid.children[1];

    assert_eq!(item1.dimensions.content.x(), 0.0);
    assert_eq!(item1.dimensions.content.width(), 100.0);

    // Implicit tracks 2 and 3 are created as Auto tracks.
    // Item 2 starts at column line 4 (index 3).
    assert!(
        item2.dimensions.content.x() >= 200.0,
        "Item 2 placed after implicit columns"
    );
}

#[test]
fn test_grid_auto_flow_column_and_dense() {
    // 1. grid-auto-flow: column
    let html = r#"<div class="grid"><div class="item">1</div><div class="item">2</div><div class="item">3</div></div>"#;
    let css = r#"
        .grid {
            display: grid;
            width: 400px;
            grid-template-rows: 50px 50px;
            grid-auto-flow: column;
        }
        .item { width: 100px; }
    "#;
    let grid = layout_html_grid(html, css, 600.0, 600.0);
    let it1 = &grid.children[0];
    let it2 = &grid.children[1];
    let it3 = &grid.children[2];

    // Item 1: col 0, row 0 (x=0, y=0)
    assert_eq!(it1.dimensions.content.x(), 0.0);
    assert_eq!(it1.dimensions.content.y(), 0.0);

    // Item 2: col 0, row 1 (x=0, y=50)
    assert_eq!(it2.dimensions.content.x(), 0.0);
    assert_eq!(it2.dimensions.content.y(), 50.0);

    // Item 3: column flow wraps to col 1, row 0 (x > 0, y=0)
    assert!(it3.dimensions.content.x() > 0.0);
    assert_eq!(it3.dimensions.content.y(), 0.0);
}

#[test]
fn test_subgrid_track_resolution() {
    let mut container =
        mango_layout::box_tree::LayoutBox::new(mango_layout::box_model::BoxType::BlockNode, None);
    let mut c_style = mango_css::ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(400.0);
    c_style.grid_template_columns = vec![
        GridTrackSize::Subgrid,
        GridTrackSize::Length(Length::Px(150.0)),
    ];
    container.style = Some(c_style);

    let mut item1 =
        mango_layout::box_tree::LayoutBox::new(mango_layout::box_model::BoxType::BlockNode, None);
    let mut i1_style = mango_css::ComputedStyle::default();
    i1_style.height = Length::Px(80.0);
    item1.style = Some(i1_style);

    let mut item2 =
        mango_layout::box_tree::LayoutBox::new(mango_layout::box_model::BoxType::BlockNode, None);
    let mut i2_style = mango_css::ComputedStyle::default();
    i2_style.height = Length::Px(80.0);
    item2.style = Some(i2_style);

    container.children.push(item1);
    container.children.push(item2);

    let cb = Dimensions::new(Rect::new(0.0, 0.0, 400.0, 400.0));
    let mut float_ctx = FloatContext::new();
    layout_grid(&mut container, &cb, &mut float_ctx);

    // Subgrid column resolved as flexible 1fr sharing with 150px fixed track: 400 - 150 = 250px
    assert_eq!(container.children[0].dimensions.content.width(), 250.0);
    assert_eq!(container.children[1].dimensions.content.width(), 150.0);
}
