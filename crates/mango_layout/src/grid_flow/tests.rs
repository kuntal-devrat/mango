use mango_core::Rect;
use mango_css::ComputedStyle;
use mango_css::values::{
    AlignContent, Display, GridPlacement, GridTrackSize, JustifyContent, Length,
};

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::container::layout_grid;

#[test]
fn test_grid_two_track_wikipedia_columns() {
    // Simulates Wikipedia 2-column layout: 200px sidebar TOC + 1fr main content, 20px gap
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(600.0);
    c_style.column_gap = Length::Px(20.0);
    c_style.grid_template_columns = vec![
        GridTrackSize::Length(Length::Px(200.0)),
        GridTrackSize::Fr(1.0),
    ];
    container.style = Some(c_style.into());

    // Child 1: Sidebar (height: 300px)
    let mut sidebar = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s_style = ComputedStyle::default();
    s_style.height = Length::Px(300.0);
    sidebar.style = Some(s_style.into());

    // Child 2: Main article content (height: 500px)
    let mut main_content = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut m_style = ComputedStyle::default();
    m_style.height = Length::Px(500.0);
    main_content.style = Some(m_style.into());

    container.children.push(sidebar);
    container.children.push(main_content);

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 600.0, 800.0);
    let mut float_ctx = FloatContext::new();

    layout_grid(&mut container, &containing_block, &mut float_ctx);

    let side = &container.children[0];
    let main = &container.children[1];

    // Sidebar at (0, 0), width 200, height 300
    assert_eq!(side.dimensions.content.x(), 0.0);
    assert_eq!(side.dimensions.content.y(), 0.0);
    assert_eq!(side.dimensions.content.width(), 200.0);
    assert_eq!(side.dimensions.content.height(), 300.0);

    // Main content at (200 + 20 = 220, 0), width 600 - 200 - 20 = 380, height 500
    assert_eq!(main.dimensions.content.x(), 220.0);
    assert_eq!(main.dimensions.content.y(), 0.0);
    assert_eq!(main.dimensions.content.width(), 380.0);
    assert_eq!(main.dimensions.content.height(), 500.0);

    // Container height is max(300, 500) = 500
    assert_eq!(container.dimensions.content.height(), 500.0);
}

#[test]
fn test_grid_item_span_and_auto_wrapping() {
    // Grid with 2 columns (1fr 1fr), container width 400px
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(400.0);
    c_style.row_gap = Length::Px(10.0);
    c_style.grid_template_columns = vec![GridTrackSize::Fr(1.0), GridTrackSize::Fr(1.0)];
    container.style = Some(c_style.into());

    // Item 1: Full-width Header spanning both columns (grid-column: 1 / 3)
    let mut header = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut h_style = ComputedStyle::default();
    h_style.height = Length::Px(60.0);
    h_style.grid_column_start = GridPlacement::Line(1);
    h_style.grid_column_end = GridPlacement::Line(3);
    header.style = Some(h_style.into());

    // Item 2: Col 0 on Row 1
    let mut item2 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut i2_style = ComputedStyle::default();
    i2_style.height = Length::Px(100.0);
    item2.style = Some(i2_style.into());

    // Item 3: Col 1 on Row 1
    let mut item3 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut i3_style = ComputedStyle::default();
    i3_style.height = Length::Px(120.0);
    item3.style = Some(i3_style.into());

    container.children.push(header);
    container.children.push(item2);
    container.children.push(item3);

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 400.0, 600.0);
    let mut float_ctx = FloatContext::new();

    layout_grid(&mut container, &containing_block, &mut float_ctx);

    let h = &container.children[0];
    let i2 = &container.children[1];
    let i3 = &container.children[2];

    // Header spans columns 0 and 1 -> width = 400
    assert_eq!(h.dimensions.content.x(), 0.0);
    assert_eq!(h.dimensions.content.y(), 0.0);
    assert_eq!(h.dimensions.content.width(), 400.0);
    assert_eq!(h.dimensions.content.height(), 60.0);

    // Item 2 and 3 placed on row 1 (y = 60 + 10 = 70)
    assert_eq!(i2.dimensions.content.x(), 0.0);
    assert_eq!(i2.dimensions.content.y(), 70.0);
    assert_eq!(i2.dimensions.content.width(), 200.0);

    assert_eq!(i3.dimensions.content.x(), 200.0);
    assert_eq!(i3.dimensions.content.y(), 70.0);
    assert_eq!(i3.dimensions.content.width(), 200.0);

    // Total grid height = 60 (header) + 10 (gap) + 120 (max(100, 120)) = 190
    assert_eq!(container.dimensions.content.height(), 190.0);
}

#[test]
fn test_grid_template_areas_named_placement() {
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(1000.0);
    c_style.column_gap = Length::Px(20.0);
    c_style.grid_template_columns = vec![
        GridTrackSize::Fr(1.0),
        GridTrackSize::Length(Length::Px(200.0)),
    ];
    c_style.grid_template_areas = vec![
        vec!["titlebar".to_string(), "columnend".to_string()],
        vec!["content".to_string(), "columnend".to_string()],
    ];
    container.style = Some(c_style.into());

    // Sidebar placed in columnend
    let mut sidebar = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s_style = ComputedStyle::default();
    s_style.grid_column_start = GridPlacement::Area("columnend".to_string());
    s_style.grid_row_start = GridPlacement::Area("columnend".to_string());
    s_style.height = Length::Px(300.0);
    sidebar.style = Some(s_style.into());

    // Titlebar placed in titlebar
    let mut titlebar = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut t_style = ComputedStyle::default();
    t_style.grid_column_start = GridPlacement::Area("titlebar".to_string());
    t_style.grid_row_start = GridPlacement::Area("titlebar".to_string());
    t_style.height = Length::Px(50.0);
    titlebar.style = Some(t_style.into());

    // Main content placed in content
    let mut content = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut m_style = ComputedStyle::default();
    m_style.grid_column_start = GridPlacement::Area("content".to_string());
    m_style.grid_row_start = GridPlacement::Area("content".to_string());
    m_style.height = Length::Px(400.0);
    content.style = Some(m_style.into());

    // Note: added in non-DOM order to verify areas place them correctly!
    container.children.push(sidebar);
    container.children.push(titlebar);
    container.children.push(content);

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 1000.0, 800.0);
    let mut float_ctx = FloatContext::new();

    layout_grid(&mut container, &containing_block, &mut float_ctx);

    let side = &container.children[0];
    let title = &container.children[1];
    let main = &container.children[2];

    // Column widths: Col 1 is 200px, Col 0 is 1000 - 200 - 20 = 780px
    // Titlebar at (0, 0), width 780, height 50
    assert_eq!(title.dimensions.content.x(), 0.0);
    assert_eq!(title.dimensions.content.y(), 0.0);
    assert_eq!(title.dimensions.content.width(), 780.0);
    assert_eq!(title.dimensions.content.height(), 50.0);

    // Main content at (0, 50), width 780, height 400
    assert_eq!(main.dimensions.content.x(), 0.0);
    assert_eq!(main.dimensions.content.y(), 50.0);
    assert_eq!(main.dimensions.content.width(), 780.0);
    assert_eq!(main.dimensions.content.height(), 400.0);

    // Sidebar at col 1 (x = 780 + 20 = 800), y = 0, width 200, height 300
    assert_eq!(side.dimensions.content.x(), 800.0);
    assert_eq!(side.dimensions.content.y(), 0.0);
    assert_eq!(side.dimensions.content.width(), 200.0);
    assert_eq!(side.dimensions.content.height(), 300.0);
}

#[test]
fn test_grid_minmax_constrained_by_content_track() {
    // Simulates Wikipedia Vector 2022 main grid:
    // grid-template-columns: minmax(0, 948px) min-content
    // Container content width: 972px, col-gap: 24px
    // Child 0 in col 0: large content
    // Child 1 in col 1: min-content sidebar (width 196px)
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(972.0);
    c_style.column_gap = Length::Px(24.0);
    c_style.grid_template_columns = vec![
        GridTrackSize::MinMax(
            Box::new(GridTrackSize::Length(Length::Px(0.0))),
            Box::new(GridTrackSize::Length(Length::Px(948.0))),
        ),
        GridTrackSize::MinContent,
    ];
    container.style = Some(c_style.into());

    let mut article = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut a_style = ComputedStyle::default();
    a_style.height = Length::Px(500.0);
    article.style = Some(a_style.into());

    let mut appearance = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut app_style = ComputedStyle::default();
    app_style.width = Length::Px(196.0);
    app_style.height = Length::Px(300.0);
    appearance.style = Some(app_style.into());

    container.children.push(article);
    container.children.push(appearance);

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 972.0, 800.0);
    let mut float_ctx = FloatContext::new();

    layout_grid(&mut container, &containing_block, &mut float_ctx);

    let art = &container.children[0];
    let app = &container.children[1];

    // Available for tracks: 972 - 24 = 948.
    // Col 1 takes 196px. Col 0 gets 948 - 196 = 752px!
    assert_eq!(art.dimensions.content.width(), 752.0);
    assert_eq!(app.dimensions.content.width(), 196.0);
    assert_eq!(app.dimensions.content.x(), 752.0 + 24.0); // 776.0
}

#[test]
fn test_grid_repeat_autofill_zero_min_size_no_oom() {
    // Test that repeat(auto-fill, ...) with 0.0 min size does not cause division by zero or OOM
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(500.0);
    c_style.column_gap = Length::Px(0.0);
    c_style.grid_template_columns = vec![GridTrackSize::RepeatAutoFill(Box::new(
        GridTrackSize::Fr(1.0),
    ))];
    container.style = Some(c_style.into());

    let mut item = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    item.style = Some(ComputedStyle::default().into());
    container.children.push(item);

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 500.0, 500.0);
    let mut float_ctx = FloatContext::new();

    layout_grid(&mut container, &containing_block, &mut float_ctx);
    assert!(container.children[0].dimensions.content.width() > 0.0);
}

#[test]
fn test_grid_order_modified_document_order() {
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(300.0);
    c_style.grid_template_columns = vec![
        GridTrackSize::Length(Length::Px(100.0)),
        GridTrackSize::Length(Length::Px(100.0)),
        GridTrackSize::Length(Length::Px(100.0)),
    ];
    container.style = Some(c_style.into());

    // Child 0: order: 2 (should be placed in track 2)
    let mut child0 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s0 = ComputedStyle::default();
    s0.order = 2;
    child0.style = Some(s0.into());

    // Child 1: order: -1 (should be placed in track 0)
    let mut child1 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s1 = ComputedStyle::default();
    s1.order = -1;
    child1.style = Some(s1.into());

    // Child 2: order: 0 (should be placed in track 1)
    let mut child2 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s2 = ComputedStyle::default();
    s2.order = 0;
    child2.style = Some(s2.into());

    container.children.push(child0);
    container.children.push(child1);
    container.children.push(child2);

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 300.0, 300.0);
    let mut float_ctx = FloatContext::new();

    layout_grid(&mut container, &containing_block, &mut float_ctx);

    // child 1 has order -1 -> placed at col 0 (x = 0)
    assert_eq!(container.children[1].dimensions.content.x(), 0.0);
    // child 2 has order 0 -> placed at col 1 (x = 100)
    assert_eq!(container.children[2].dimensions.content.x(), 100.0);
    // child 0 has order 2 -> placed at col 2 (x = 200)
    assert_eq!(container.children[0].dimensions.content.x(), 200.0);
}

#[test]
fn test_grid_implicit_column_track_expansion() {
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(400.0);
    c_style.grid_template_columns = vec![GridTrackSize::Length(Length::Px(50.0))];
    container.style = Some(c_style.into());

    // Place an item at line 4 (grid-column: 4 / 5 -> track index 3)
    let mut item = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut i_style = ComputedStyle::default();
    i_style.grid_column_start = GridPlacement::Line(4);
    i_style.grid_column_end = GridPlacement::Line(5);
    i_style.width = Length::Px(60.0);
    item.style = Some(i_style.into());
    container.children.push(item);

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 400.0, 300.0);
    let mut float_ctx = FloatContext::new();

    layout_grid(&mut container, &containing_block, &mut float_ctx);

    // The item should be placed in track 3 (after tracks 0, 1, 2)
    // Track 0 is 50px, Tracks 1 and 2 are 0px (empty auto tracks), Track 3 is 60px
    assert_eq!(container.children[0].dimensions.content.x(), 50.0);
    assert_eq!(container.children[0].dimensions.content.width(), 60.0);
}

#[test]
fn test_grid_reverse_span_and_swapped_lines() {
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(400.0);
    c_style.grid_template_columns = vec![
        GridTrackSize::Length(Length::Px(100.0)),
        GridTrackSize::Length(Length::Px(100.0)),
        GridTrackSize::Length(Length::Px(100.0)),
        GridTrackSize::Length(Length::Px(100.0)),
    ];
    container.style = Some(c_style.into());

    // Item 1: grid-column: span 2 / 4 (ends at line 4, track index 3, starts at track 3 - 2 = 1)
    let mut item1 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s1 = ComputedStyle::default();
    s1.grid_column_start = GridPlacement::Span(2);
    s1.grid_column_end = GridPlacement::Line(4);
    item1.style = Some(s1.into());

    // Item 2: grid-column: 4 / 2 (swapped to start at line 2 = track 1, span 2)
    let mut item2 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut s2 = ComputedStyle::default();
    s2.grid_column_start = GridPlacement::Line(4);
    s2.grid_column_end = GridPlacement::Line(2);
    item2.style = Some(s2.into());

    container.children.push(item1);
    container.children.push(item2);

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 400.0, 400.0);
    let mut float_ctx = FloatContext::new();

    layout_grid(&mut container, &containing_block, &mut float_ctx);

    // Item 1 spans tracks 1 and 2 (x = 100, width = 200)
    assert_eq!(container.children[0].dimensions.content.x(), 100.0);
    assert_eq!(container.children[0].dimensions.content.width(), 200.0);

    // Item 2 also spans tracks 1 and 2 (x = 100, width = 200)
    assert_eq!(container.children[1].dimensions.content.x(), 100.0);
    assert_eq!(container.children[1].dimensions.content.width(), 200.0);
}

#[test]
fn test_grid_justify_content_and_align_content() {
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(600.0);
    c_style.height = Length::Px(400.0);
    c_style.justify_content = JustifyContent::Center;
    c_style.align_content = AlignContent::Center;
    c_style.grid_template_columns = vec![
        GridTrackSize::Length(Length::Px(100.0)),
        GridTrackSize::Length(Length::Px(100.0)),
    ];
    c_style.grid_template_rows = vec![
        GridTrackSize::Length(Length::Px(100.0)),
        GridTrackSize::Length(Length::Px(100.0)),
    ];
    container.style = Some(c_style.into());

    let mut item = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    item.style = Some(ComputedStyle::default().into());
    container.children.push(item);

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 600.0, 400.0);
    let mut float_ctx = FloatContext::new();

    layout_grid(&mut container, &containing_block, &mut float_ctx);

    // Tracks width = 200px. Container width = 600px. Free inline space = 400px.
    // justify-content: center -> offset_x = 200px.
    assert_eq!(container.children[0].dimensions.content.x(), 200.0);

    // Tracks height = 200px. Container height = 400px. Free block space = 200px.
    // align-content: center -> offset_y = 100px.
    assert_eq!(container.children[0].dimensions.content.y(), 100.0);
}

#[test]
fn test_grid_item_auto_margins() {
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(400.0);
    c_style.height = Length::Px(300.0);
    c_style.grid_template_columns = vec![GridTrackSize::Length(Length::Px(400.0))];
    c_style.grid_template_rows = vec![GridTrackSize::Length(Length::Px(300.0))];
    container.style = Some(c_style.into());

    let mut item = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut i_style = ComputedStyle::default();
    i_style.width = Length::Px(100.0);
    i_style.height = Length::Px(100.0);
    i_style.margin_left = Length::Auto;
    i_style.margin_right = Length::Auto;
    i_style.margin_top = Length::Auto;
    i_style.margin_bottom = Length::Auto;
    item.style = Some(i_style.into());
    container.children.push(item);

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 400.0, 300.0);
    let mut float_ctx = FloatContext::new();

    layout_grid(&mut container, &containing_block, &mut float_ctx);

    // Remaining width = 400 - 100 = 300. margin auto centers horizontally -> x = 150
    assert_eq!(container.children[0].dimensions.content.x(), 150.0);
    // Remaining height = 300 - 100 = 200. margin auto centers vertically -> y = 100
    assert_eq!(container.children[0].dimensions.content.y(), 100.0);
}

#[test]
fn test_grid_definite_height_row_fr() {
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(400.0);
    c_style.height = Length::Px(320.0);
    c_style.row_gap = Length::Px(20.0);
    c_style.grid_template_columns = vec![GridTrackSize::Fr(1.0)];
    c_style.grid_template_rows = vec![GridTrackSize::Fr(1.0), GridTrackSize::Fr(2.0)];
    container.style = Some(c_style.into());

    let mut item1 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    item1.style = Some(ComputedStyle::default().into());
    let mut item2 = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    item2.style = Some(ComputedStyle::default().into());

    container.children.push(item1);
    container.children.push(item2);

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 400.0, 320.0);
    let mut float_ctx = FloatContext::new();

    layout_grid(&mut container, &containing_block, &mut float_ctx);

    // Total height = 320, gap = 20 -> remaining = 300.
    // Row 0 is 1fr -> 100px. Row 1 is 2fr -> 200px.
    // Item 1 starts at y = 0, stretched to 100px
    assert_eq!(container.children[0].dimensions.content.y(), 0.0);
    assert_eq!(container.children[0].dimensions.content.height(), 100.0);

    // Item 2 starts at y = 100 + 20 = 120, stretched to 200px
    assert_eq!(container.children[1].dimensions.content.y(), 120.0);
    assert_eq!(container.children[1].dimensions.content.height(), 200.0);
}

#[test]
fn test_grid_min_max_sizing_constraints() {
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Grid;
    c_style.width = Length::Px(100.0);
    c_style.min_width = Length::Px(300.0);
    c_style.max_width = Length::Px(500.0);
    c_style.height = Length::Px(50.0);
    c_style.min_height = Length::Px(200.0);
    container.style = Some(c_style.into());

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 800.0, 800.0);
    let mut float_ctx = FloatContext::new();

    layout_grid(&mut container, &containing_block, &mut float_ctx);

    // Width clamped to min-width 300
    assert_eq!(container.dimensions.content.width(), 300.0);
    // Height clamped to min-height 200
    assert_eq!(container.dimensions.content.height(), 200.0);
}

#[test]
fn test_flex_min_greater_than_max_no_panic() {
    let mut container = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut c_style = ComputedStyle::default();
    c_style.display = Display::Flex;
    c_style.width = Length::Px(500.0);
    c_style.height = Length::Px(200.0);
    container.style = Some(c_style.into());

    let mut item = LayoutBox::new(crate::box_model::BoxType::BlockNode, None);
    let mut i_style = ComputedStyle::default();
    i_style.min_width = Length::Px(42.0);
    i_style.max_width = Length::Px(20.0);
    item.style = Some(i_style.into());
    container.children.push(item);

    let mut containing_block = Dimensions::default();
    containing_block.content = Rect::new(0.0, 0.0, 800.0, 800.0);
    let mut float_ctx = FloatContext::new();

    // Must not panic with min > max in clamp; min-width wins per CSS Flexbox §4.5
    crate::flex_flow::layout_flex(&mut container, &containing_block, &mut float_ctx);
    assert!(container.children[0].dimensions.content.width() >= 42.0);
}
