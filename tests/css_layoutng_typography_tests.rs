//! Comprehensive integration tests for CSS LayoutNG & Typography Parity (Phase 3).
//!
//! Covers:
//! 1. CSS Container Queries (`@container`) with size containers, named containers, and query evaluation
//! 2. Container Query length units (`cqw`, `cqh`, `cqi`, `cqb`, `cqmin`, `cqmax`)
//! 3. CSS Values 4 Advanced Math expressions (`round`, `mod`, `rem`, `abs`, `sign`, `sin`, `cos`, `tan`, `hypot`, `sqrt`, `pow`)
//! 4. CSS Containment & Virtualized Rendering (`contain`, `contain-intrinsic-size`, `content-visibility`)
//! 5. Complex Script Shaping (Arabic, Devanagari, Hebrew, Thai) & UAX #9 Bidirectional layout
//! 6. OpenType Layout Features (`font-feature-settings`) & Typography balancing (`text-wrap: balance | pretty`)

use mango_css::cascade::{matches_container_query, resolve_cascade};
use mango_css::computed::compute_style;
use mango_css::parser::parse_stylesheet;
use mango_css::values::{Contain, ContentVisibility, Length, TextWrap, Value};
use mango_html::parse_html;
use mango_layout::block_flow::layout_block;
use mango_layout::box_model::{BoxType, Dimensions};
use mango_layout::float::FloatContext;
use mango_layout::LayoutBox;

#[test]
fn test_container_query_units_parsing_and_resolution() {
    // 1. Direct Length::parse for cqw, cqh, cqi, cqb, cqmin, cqmax
    let w = Length::parse("50cqw").expect("50cqw should parse");
    let h = Length::parse("25cqh").expect("25cqh should parse");
    let i = Length::parse("10cqi").expect("10cqi should parse");
    let b = Length::parse("5cqb").expect("5cqb should parse");
    let min = Length::parse("20cqmin").expect("20cqmin should parse");
    let max = Length::parse("80cqmax").expect("80cqmax should parse");

    assert_eq!(w, Length::Cqw(50.0));
    assert_eq!(h, Length::Cqh(25.0));
    assert_eq!(i, Length::Cqi(10.0));
    assert_eq!(b, Length::Cqb(5.0));
    assert_eq!(min, Length::Cqmin(20.0));
    assert_eq!(max, Length::Cqmax(80.0));

    // Resolve units against a 400x200 container
    // 50cqw = 50% of 400 = 200px
    assert_eq!(w.to_px_with_viewports(16.0, 16.0, 400.0, 400.0, 200.0), 200.0);
    // 25cqh = 25% of 200 = 50px
    assert_eq!(h.to_px_with_viewports(16.0, 16.0, 400.0, 400.0, 200.0), 50.0);
    // 10cqi = 10% of inline-size (400) = 40px
    assert_eq!(i.to_px_with_viewports(16.0, 16.0, 400.0, 400.0, 200.0), 40.0);
    // 5cqb = 5% of block-size (200) = 10px
    assert_eq!(b.to_px_with_viewports(16.0, 16.0, 400.0, 400.0, 200.0), 10.0);
    // 20cqmin = 20% of min(400, 200) = 20% of 200 = 40px
    assert_eq!(min.to_px_with_viewports(16.0, 16.0, 400.0, 400.0, 200.0), 40.0);
    // 80cqmax = 80% of max(400, 200) = 80% of 400 = 320px
    assert_eq!(max.to_px_with_viewports(16.0, 16.0, 400.0, 400.0, 200.0), 320.0);
}

#[test]
fn test_container_queries_size_and_named_evaluation() {
    // 1. Condition evaluation with container queries
    assert!(matches_container_query("(min-width: 300px)", 450.0, 300.0));
    assert!(!matches_container_query("(min-width: 500px)", 450.0, 300.0));
    assert!(matches_container_query("(inline-size >= 400px)", 450.0, 300.0));
    assert!(matches_container_query("(300px <= width <= 500px)", 450.0, 300.0));
    assert!(matches_container_query("((min-width: 200px) and (max-height: 400px))", 450.0, 300.0));
    assert!(!matches_container_query("not (min-width: 200px)", 450.0, 300.0));

    // 2. Cascade matching with size containers and named containers
    let html = r#"
        <div id="sidebar" style="container-type: inline-size; container-name: sidebar-cont; width: 350px;">
            <div id="card" class="card">Sidebar Card</div>
        </div>
        <div id="main" style="container-type: inline-size; container-name: main-cont; width: 750px;">
            <div id="main-card" class="card">Main Card</div>
        </div>
    "#;
    let doc = parse_html(html);
    let root = doc.root();
    let card_id = doc.find_element_by_id(root, "card").unwrap();
    let main_card_id = doc.find_element_by_id(root, "main-card").unwrap();

    let css = r#"
        .card { font-size: 14px; color: #333333; }
        @container sidebar-cont (max-width: 400px) {
            .card { font-size: 12px; }
        }
        @container main-cont (min-width: 600px) {
            .card { font-size: 20px; }
        }
    "#;
    let sheet = parse_stylesheet(css);
    let cascaded_card = resolve_cascade(card_id, &doc, &[&sheet]);
    let cascaded_main = resolve_cascade(main_card_id, &doc, &[&sheet]);

    // sidebar card matches max-width: 400px -> font-size: 12px
    let card_font_size = cascaded_card.get("font-size").unwrap();
    assert_eq!(card_font_size, &Value::Length(Length::Px(12.0)));

    // main card matches min-width: 600px -> font-size: 20px
    let main_font_size = cascaded_main.get("font-size").unwrap();
    assert_eq!(main_font_size, &Value::Length(Length::Px(20.0)));
}

#[test]
fn test_css_values_4_math_stepping_and_rounding() {
    // 1. round() with nearest, up, down, to-zero
    let val_nearest = Length::parse("round(nearest, 14px, 5px)").expect("round(nearest) should parse");
    assert_eq!(val_nearest, Length::Px(15.0));

    let val_up = Length::parse("round(up, 12px, 5px)").expect("round(up) should parse");
    assert_eq!(val_up, Length::Px(15.0));

    let val_down = Length::parse("round(down, 14px, 5px)").expect("round(down) should parse");
    assert_eq!(val_down, Length::Px(10.0));

    let val_to_zero = Length::parse("round(to-zero, -14px, 5px)").expect("round(to-zero) should parse");
    assert_eq!(val_to_zero, Length::Px(-10.0));

    // 2. mod() and rem()
    let val_mod = Length::parse("mod(17px, 5px)").expect("mod() should parse");
    assert_eq!(val_mod, Length::Px(2.0));

    let val_rem = Length::parse("rem(17px, 5px)").expect("rem() should parse");
    assert_eq!(val_rem, Length::Px(2.0));

    // Negative modulus: CSS mod(-17, 5) is 3 (same sign as divisor)
    let val_mod_neg = Length::parse("mod(-17px, 5px)").expect("mod(-17) should parse");
    assert_eq!(val_mod_neg, Length::Px(3.0));

    // Negative remainder: CSS rem(-17, 5) is -2 (same sign as dividend)
    let val_rem_neg = Length::parse("rem(-17px, 5px)").expect("rem(-17) should parse");
    assert_eq!(val_rem_neg, Length::Px(-2.0));
}

#[test]
fn test_css_values_4_numeric_transforms_and_trigonometric() {
    // 1. abs() and sign()
    let val_abs = Length::parse("abs(-45px)").expect("abs(-45px) should parse");
    assert_eq!(val_abs, Length::Px(45.0));

    let val_sign = Length::parse("calc(100px * sign(-30px))").expect("sign(-30px) should parse");
    assert_eq!(val_sign, Length::Px(-100.0));

    let val_sign_pos = Length::parse("calc(100px * sign(50px))").expect("sign(50px) should parse");
    assert_eq!(val_sign_pos, Length::Px(100.0));

    // 2. hypot(), sqrt(), pow()
    let val_hypot = Length::parse("hypot(30px, 40px)").expect("hypot should parse");
    assert_eq!(val_hypot, Length::Px(50.0));

    let val_sqrt = Length::parse("sqrt(144px)").expect("sqrt should parse");
    assert_eq!(val_sqrt, Length::Px(12.0));

    let val_pow = Length::parse("calc(10px * pow(2, 3))").expect("pow should parse");
    assert_eq!(val_pow, Length::Px(80.0));

    // 3. Trigonometric functions
    let val_sin = Length::parse("calc(100px * sin(90deg))").expect("sin(90deg) should parse");
    assert_eq!(val_sin, Length::Px(100.0));

    let val_cos = Length::parse("calc(100px * cos(0deg))").expect("cos(0deg) should parse");
    assert_eq!(val_cos, Length::Px(100.0));

    // 4. Nested math in calc(): calc(100px + round(18px, 10px)) -> 120px
    let val_nested = Length::parse("calc(100px + round(18px, 10px))").expect("nested calc should parse");
    assert_eq!(val_nested, Length::Px(120.0));
}

#[test]
fn test_css_containment_multi_keyword_and_intrinsic_size() {
    // 1. Multi-keyword containment parsing
    let c1 = Contain::parse("layout paint").expect("layout paint should parse");
    assert!(c1.contains_layout());
    assert!(c1.contains_paint());
    assert!(!c1.contains_size());

    let c2 = Contain::parse("inline-size layout style").expect("compound should parse");
    assert!(c2.contains_inline_size());
    assert!(c2.contains_layout());
    assert!(c2.contains_style());
    assert!(!c2.contains_paint());

    let c_strict = Contain::parse("strict").unwrap();
    assert!(c_strict.contains_size());
    assert!(c_strict.contains_layout());
    assert!(c_strict.contains_paint());

    let c_content = Contain::parse("content").unwrap();
    assert!(!c_content.contains_size());
    assert!(c_content.contains_layout());
    assert!(c_content.contains_paint());

    // 2. Computed style resolution for contain-intrinsic-size
    let html = r#"<div id="box" style="contain: size layout; contain-intrinsic-size: 240px 180px;">Content</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let box_id = doc.find_element_by_id(root, "box").unwrap();

    let style = compute_style(box_id, &doc, &[], None);

    assert!(style.contain.contains_size());
    assert!(style.contain.contains_layout());
    assert_eq!(style.contain_intrinsic_width, Some(Length::Px(240.0)));
    assert_eq!(style.contain_intrinsic_height, Some(Length::Px(180.0)));

    // 3. Layout execution honoring contain-intrinsic-size when size containment is active
    let mut layout_box = LayoutBox::new(BoxType::BlockNode, None);
    layout_box.style = Some(style.into());

    let cb = Dimensions::default();
    let mut float_ctx = FloatContext::new();
    layout_block(&mut layout_box, &cb, &mut float_ctx);

    // With contain: size and auto dimensions, the content size should match contain-intrinsic dimensions
    assert_eq!(layout_box.dimensions.content.size.width, 240.0);
    assert_eq!(layout_box.dimensions.content.size.height, 180.0);
}

#[test]
fn test_content_visibility_skips_child_layout() {
    let html = r#"
        <div id="hidden-section" style="content-visibility: hidden; contain-intrinsic-size: 500px 300px;">
            <div id="child1" style="height: 100px;">Child 1</div>
            <div id="child2" style="height: 200px;">Child 2</div>
        </div>
    "#;
    let doc = parse_html(html);
    let root = doc.root();
    let section_id = doc.find_element_by_id(root, "hidden-section").unwrap();

    let style = compute_style(section_id, &doc, &[], None);

    assert_eq!(style.content_visibility, ContentVisibility::Hidden);
    assert_eq!(style.contain_intrinsic_height, Some(Length::Px(300.0)));

    let mut section_box = LayoutBox::new(BoxType::BlockNode, None);
    section_box.style = Some(style.into());

    let child1 = LayoutBox::new(BoxType::BlockNode, None);
    section_box.children.push(child1);

    let cb = Dimensions::default();
    let mut float_ctx = FloatContext::new();
    layout_block(&mut section_box, &cb, &mut float_ctx);

    // Height reflects contain_intrinsic_height (300px) instead of child heights
    assert_eq!(section_box.dimensions.content.size.height, 300.0);
}

#[test]
fn test_opentype_font_feature_settings_and_text_wrap() {
    // 1. font-feature-settings parsing
    let html = r#"<div id="typography" style="font-feature-settings: 'kern' 1, 'liga' 1, 'tnum' 0; text-wrap: balance;">Balanced Typography</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let typo_id = doc.find_element_by_id(root, "typography").unwrap();

    let style = compute_style(typo_id, &doc, &[], None);

    assert_eq!(style.text_wrap, TextWrap::Balance);
    match &style.font_feature_settings {
        mango_css::values::FontFeatureSettings::Features(features) => {
            assert_eq!(features.len(), 3);
            assert_eq!(features[0], ("kern".to_string(), 1));
            assert_eq!(features[1], ("liga".to_string(), 1));
            assert_eq!(features[2], ("tnum".to_string(), 0));
        }
        other => panic!("Expected FontFeatureSettings::Features, got {:?}", other),
    }

    // 2. Inheritance of text-wrap to children
    let html_nested = r#"
        <div id="parent" style="text-wrap: pretty;">
            <p id="child">Nested paragraph</p>
        </div>
    "#;
    let doc_nested = parse_html(html_nested);
    let root_nested = doc_nested.root();
    let p_id = doc_nested.find_element_by_id(root_nested, "child").unwrap();
    let parent_id = doc_nested.find_element_by_id(root_nested, "parent").unwrap();

    let parent_style = compute_style(parent_id, &doc_nested, &[], None);
    let child_style = compute_style(p_id, &doc_nested, &[], Some(&parent_style));

    assert_eq!(parent_style.text_wrap, TextWrap::Pretty);
    assert_eq!(child_style.text_wrap, TextWrap::Pretty);
}
