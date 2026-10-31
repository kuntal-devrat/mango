fn wpt_flexbox_path(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/wpt/upstream/css/css-flexbox")
        .join(rel)
}

#[test]
fn test_flexbox_inline_negative_margin() {
    let test_html = std::fs::read_to_string(wpt_flexbox_path("flex-inline.html")).unwrap();
    let ref_html =
        std::fs::read_to_string(wpt_flexbox_path("reference/flex-inline-ref.html")).unwrap();
    let doc = mango_html::parse_html(&test_html);
    let (box_t, _) = mango_layout::layout_document_with_scroll_xy(
        &doc,
        &[],
        mango_core::Size::new(800.0, 600.0),
        0.0,
        0.0,
    );
    let doc_r = mango_html::parse_html(&ref_html);
    let (box_r, _) = mango_layout::layout_document_with_scroll_xy(
        &doc_r,
        &[],
        mango_core::Size::new(800.0, 600.0),
        0.0,
        0.0,
    );
    let container_t = &box_t.children[0].children[1];
    let container_r = &box_r.children[0].children[1];
    let sq_t = &container_t.children[0];
    let sq_r = &container_r.children[0];
    assert_eq!(sq_t.dimensions.content, sq_r.dimensions.content);
}

#[test]
fn test_flex_vertical_align_effect() {
    let test_html =
        std::fs::read_to_string(wpt_flexbox_path("flex-vertical-align-effect.html")).unwrap();
    let ref_html = std::fs::read_to_string(wpt_flexbox_path(
        "reference/flex-vertical-align-effect.html",
    ))
    .unwrap();

    let doc_t = mango_html::parse_html(&test_html);
    let (box_t, _) = mango_layout::layout_document_with_scroll_xy(
        &doc_t,
        &[],
        mango_core::Size::new(800.0, 600.0),
        0.0,
        0.0,
    );

    let doc_r = mango_html::parse_html(&ref_html);
    let (box_r, _) = mango_layout::layout_document_with_scroll_xy(
        &doc_r,
        &[],
        mango_core::Size::new(800.0, 600.0),
        0.0,
        0.0,
    );

    let c_t = &box_t.children[0].children[0];
    let c_r = &box_r.children[0].children[0];
    assert_eq!(
        c_t.children[1].dimensions.content.origin.y,
        c_r.children[1].dimensions.content.origin.y
    );
    assert_eq!(
        c_t.children[0].dimensions.content.size.width,
        c_r.children[0].dimensions.content.size.width
    );
    assert_eq!(
        c_t.children[0].dimensions.border_box().width(),
        c_r.children[0].dimensions.border_box().width()
    );
}

#[test]
fn test_flexbox_baseline_multi_line_vert_001() {
    let test_html = std::fs::read_to_string(wpt_flexbox_path(
        "flexbox-baseline-multi-line-vert-001.html",
    ))
    .unwrap();
    let ref_html = std::fs::read_to_string(wpt_flexbox_path(
        "flexbox-baseline-multi-line-vert-001-ref.html",
    ))
    .unwrap();
    let doc_t = mango_html::parse_html(&test_html);
    let (box_t, _) = mango_layout::layout_document_with_scroll_xy(
        &doc_t,
        &[],
        mango_core::Size::new(800.0, 600.0),
        0.0,
        0.0,
    );
    let doc_r = mango_html::parse_html(&ref_html);
    let (box_r, _) = mango_layout::layout_document_with_scroll_xy(
        &doc_r,
        &[],
        mango_core::Size::new(800.0, 600.0),
        0.0,
        0.0,
    );
    let body_t = &box_t.children[0];
    let body_r = &box_r.children[0];
    for (i, c) in body_t.children.iter().enumerate() {
        eprintln!(
            "test child {}: type={:?} tag={:?} content={:?}",
            i, c.box_type, c.tag_name, c.dimensions.content
        );
        for (j, sub) in c.children.iter().enumerate() {
            eprintln!(
                "  test sub {}: type={:?} tag={:?} content={:?}",
                j, sub.box_type, sub.tag_name, sub.dimensions.content
            );
        }
    }
    for (i, c) in body_r.children.iter().enumerate() {
        eprintln!(
            "ref child {}: type={:?} tag={:?} content={:?}",
            i, c.box_type, c.tag_name, c.dimensions.content
        );
        for (j, sub) in c.children.iter().enumerate() {
            eprintln!(
                "  ref sub {}: type={:?} tag={:?} content={:?}",
                j, sub.box_type, sub.tag_name, sub.dimensions.content
            );
        }
    }
}

#[test]
fn test_negative_margins_001() {
    let test_html = std::fs::read_to_string(wpt_flexbox_path("negative-margins-001.html")).unwrap();
    let ref_html =
        std::fs::read_to_string(wpt_flexbox_path("reference/negative-margins-001-ref.html"))
            .unwrap();
    let doc_t = mango_html::parse_html(&test_html);
    let (box_t, _) = mango_layout::layout_document_with_scroll_xy(
        &doc_t,
        &[],
        mango_core::Size::new(800.0, 600.0),
        0.0,
        0.0,
    );
    let doc_r = mango_html::parse_html(&ref_html);
    let (box_r, _) = mango_layout::layout_document_with_scroll_xy(
        &doc_r,
        &[],
        mango_core::Size::new(800.0, 600.0),
        0.0,
        0.0,
    );
    let body_t = &box_t.children[0];
    let body_r = &box_r.children[0];
    assert_eq!(
        body_t.children[1].dimensions.content.size.height,
        body_r.children[1].dimensions.content.size.height
    );
}

#[test]
fn test_flexbox_baseline_multi_item_horiz_001a() {
    let test_html = std::fs::read_to_string(wpt_flexbox_path(
        "flexbox-baseline-multi-item-horiz-001a.html",
    ))
    .unwrap();
    let ref_html = std::fs::read_to_string(wpt_flexbox_path(
        "flexbox-baseline-multi-item-horiz-001-ref.html",
    ))
    .unwrap();
    let doc_t = mango_html::parse_html(&test_html);
    let (box_t, _) = mango_layout::layout_document_with_scroll_xy(
        &doc_t,
        &[],
        mango_core::Size::new(800.0, 600.0),
        0.0,
        0.0,
    );
    let doc_r = mango_html::parse_html(&ref_html);
    let (box_r, _) = mango_layout::layout_document_with_scroll_xy(
        &doc_r,
        &[],
        mango_core::Size::new(800.0, 600.0),
        0.0,
        0.0,
    );
    let body_t = &box_t.children[0];
    let body_r = &box_r.children[0];
    for (i, c) in body_t.children.iter().enumerate() {
        eprintln!(
            "t child {}: tag={:?} box_type={:?} y={} h={} attr={:?}",
            i,
            c.tag_name,
            c.box_type,
            c.dimensions.content.y(),
            c.dimensions.content.height(),
            c.attributes
        );
        for (j, sub) in c.children.iter().enumerate() {
            eprintln!(
                "  t sub {}: tag={:?} box_type={:?} y={} h={}",
                j,
                sub.tag_name,
                sub.box_type,
                sub.dimensions.content.y(),
                sub.dimensions.content.height()
            );
        }
    }
    eprintln!(
        "t child 1 baseline={:?}",
        mango_layout::inline_flow::baseline::find_inline_block_last_baseline_from_top(
            &body_t.children[1]
        )
    );
    eprintln!(
        "r child 1 baseline={:?}",
        mango_layout::inline_flow::baseline::find_inline_block_last_baseline_from_top(
            &body_r.children[1]
        )
    );
}

#[test]
fn test_flexbox_baseline_nested_001() {
    let test_html =
        std::fs::read_to_string(wpt_flexbox_path("flexbox-baseline-nested-001.html")).unwrap();
    let ref_html =
        std::fs::read_to_string(wpt_flexbox_path("flexbox-baseline-nested-001-ref.html")).unwrap();
    let doc_t = mango_html::parse_html(&test_html);
    let (box_t, _) = mango_layout::layout_document_with_scroll_xy(
        &doc_t,
        &[],
        mango_core::Size::new(800.0, 600.0),
        0.0,
        0.0,
    );
    let doc_r = mango_html::parse_html(&ref_html);
    let (box_r, _) = mango_layout::layout_document_with_scroll_xy(
        &doc_r,
        &[],
        mango_core::Size::new(800.0, 600.0),
        0.0,
        0.0,
    );
    let ib_t = &box_t.children[0].children[1];
    let ib_r = &box_r.children[0].children[1];
    eprintln!(
        "ib_t: content={:?} attr={:?}",
        ib_t.dimensions.content, ib_t.attributes
    );
    eprintln!(
        "ib_r: content={:?} attr={:?}",
        ib_r.dimensions.content, ib_r.attributes
    );
    let outer_t = &ib_t.children[0];
    let inner_t = &outer_t.children[0];
    let input_t = &inner_t.children[0];
    let inner_r = &ib_r.children[0];
    let input_r = &inner_r.children[0];
    let mut memo = std::collections::HashMap::new();
    let (c_min, c_max) = mango_layout::table_flow::measure_box_content_intrinsic_widths(
        inner_t,
        16.0,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
        &mut memo,
    );
    eprintln!(
        "inner_t measure_box_content_intrinsic_widths: c_min={}, c_max={}",
        c_min, c_max
    );
    eprintln!("outer_t: content={:?}", outer_t.dimensions.content);
    eprintln!("inner_t: content={:?}", inner_t.dimensions.content);
    eprintln!(
        "input_t: content={:?} box_sizing={:?}",
        input_t.dimensions.content,
        input_t.style.as_ref().map(|s| s.box_sizing)
    );
    eprintln!("inner_r: content={:?}", inner_r.dimensions.content);
    eprintln!(
        "input_r: content={:?} box_sizing={:?}",
        input_r.dimensions.content,
        input_r.style.as_ref().map(|s| s.box_sizing)
    );
    assert_eq!(ib_t.dimensions.content, ib_r.dimensions.content);
    assert_eq!(inner_t.dimensions.content, inner_r.dimensions.content);
    assert_eq!(input_t.dimensions.content, input_r.dimensions.content);
}
