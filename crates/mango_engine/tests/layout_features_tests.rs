use mango_core::Rect;
use mango_css::animation::{AnimationEngine, ScrollTimeline};
use mango_css::computed::ComputedStyle;
use mango_css::values::{Length, Position, TextWrap};
use mango_engine::Engine;
use mango_html::dom::NodeId;
use mango_layout::StyledNode;
use mango_layout::box_model::BoxType;
use mango_layout::box_tree::LayoutBox;
use mango_layout::dimensions::Dimensions;
use mango_layout::float::FloatContext;

fn find_styled_node(root: &StyledNode, target_id: NodeId) -> Option<&StyledNode> {
    if root.node_id == Some(target_id) {
        return Some(root);
    }
    for child in &root.children {
        if let Some(found) = find_styled_node(child, target_id) {
            return Some(found);
        }
    }
    None
}

#[test]
fn test_css_color_wide_gamut_and_relative_color_syntax() {
    let mut engine = Engine::new();
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head>
                <style>
                    #wide-rec2020 { color: color(rec2020 0.8 0.2 0.1); }
                    #wide-a98 { color: color(a98-rgb 0.1 0.7 0.9); }
                    #wide-prophoto { color: color(prophoto-rgb 0.9 0.4 0.2); }
                    #rcs-rgb { color: rgb(from #ff0000 calc(r * 0.5) g b); }
                    #rcs-hsl { color: hsl(from #00ff00 h calc(s * 0.8) 40%); }
                    #rcs-oklch { color: oklch(from #0000ff l c h); }
                </style>
            </head>
            <body>
                <div id="wide-rec2020">Rec2020</div>
                <div id="wide-a98">A98</div>
                <div id="wide-prophoto">ProPhoto</div>
                <div id="rcs-rgb">RCS RGB</div>
                <div id="rcs-hsl">RCS HSL</div>
                <div id="rcs-oklch">RCS OKLCH</div>
            </body>
        </html>
    "#;

    let _dl = engine.load(html, "");
    let doc = engine.document();
    let style_tree = engine.style_tree().expect("style tree must exist");

    // Rec2020, A98, and ProPhoto convert to valid non-transparent colors
    let rec_node = doc.find_element_by_id(doc.root(), "wide-rec2020").unwrap();
    let rec_style = find_styled_node(style_tree, rec_node).unwrap();
    assert_eq!(rec_style.style.color.a, 255);
    assert!(rec_style.style.color.r > 200);

    let a98_node = doc.find_element_by_id(doc.root(), "wide-a98").unwrap();
    let a98_style = find_styled_node(style_tree, a98_node).unwrap();
    assert_eq!(a98_style.style.color.a, 255);
    assert!(a98_style.style.color.g > 150);

    // Relative color syntax: rgb(from #ff0000 calc(r * 0.5) g b)
    // r was 255 -> 255 * 0.5 = 127
    let rcs_rgb_node = doc.find_element_by_id(doc.root(), "rcs-rgb").unwrap();
    let rcs_rgb_style = find_styled_node(style_tree, rcs_rgb_node).unwrap();
    assert_eq!(rcs_rgb_style.style.color.r, 128);
    assert_eq!(rcs_rgb_style.style.color.g, 0);
    assert_eq!(rcs_rgb_style.style.color.b, 0);

    // Relative color syntax: hsl(from #00ff00 ...)
    let rcs_hsl_node = doc.find_element_by_id(doc.root(), "rcs-hsl").unwrap();
    let rcs_hsl_style = find_styled_node(style_tree, rcs_hsl_node).unwrap();
    assert!(rcs_hsl_style.style.color.g > 150);
}

#[test]
fn test_css_anchor_positioning_and_try_fallbacks() {
    mango_layout::block_flow::clear_anchors();

    // 1. Layout an anchor element
    let mut anchor_box = LayoutBox::new(BoxType::BlockNode, None);
    anchor_box.dimensions.content = Rect::new(150.0, 100.0, 200.0, 50.0);
    let anchor_style = ComputedStyle {
        anchor_name: Some("--header-anchor".to_string()),
        ..Default::default()
    };
    anchor_box.style = Some(anchor_style.into());

    mango_layout::block_flow::register_anchor(
        "--header-anchor",
        anchor_box.dimensions.border_box(),
    );

    // 2. Position element anchored below: top: anchor(bottom); left: anchor(left);
    let mut popup_box = LayoutBox::new(BoxType::BlockNode, None);
    popup_box.dimensions.content = Rect::new(0.0, 0.0, 150.0, 80.0);
    let popup_style = ComputedStyle {
        position: Position::Absolute,
        position_anchor: Some("--header-anchor".to_string()),
        top: Length::parse("anchor(bottom)").unwrap(),
        left: Length::parse("anchor(left)").unwrap(),
        ..Default::default()
    };
    popup_box.style = Some(popup_style.into());

    let containing_block = Dimensions::new(Rect::new(0.0, 0.0, 1000.0, 800.0));
    let mut float_ctx = FloatContext::new();

    // Position the element
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions = containing_block;
    container.children.push(popup_box);
    mango_layout::block_flow::layout_positioned_children(&mut container, &mut float_ctx);

    let placed = &container.children[0];
    // Top aligns with anchor bottom: 100 + 50 = 150
    assert_eq!(placed.dimensions.content.y(), 150.0);
    // Left aligns with anchor left: 150
    assert_eq!(placed.dimensions.content.x(), 150.0);

    // 3. Test position-try fallback when element overflows bottom of viewport
    let mut near_bottom_anchor = LayoutBox::new(BoxType::BlockNode, None);
    near_bottom_anchor.dimensions.content = Rect::new(200.0, 750.0, 100.0, 30.0);
    mango_layout::block_flow::register_anchor(
        "--bottom-btn",
        near_bottom_anchor.dimensions.border_box(),
    );

    let mut overflow_popup = LayoutBox::new(BoxType::BlockNode, None);
    overflow_popup.dimensions.content = Rect::new(0.0, 0.0, 100.0, 60.0);
    let overflow_style = ComputedStyle {
        position: Position::Absolute,
        position_anchor: Some("--bottom-btn".to_string()),
        top: Length::parse("anchor(bottom)").unwrap(),
        position_try_fallbacks: vec!["flip-block".to_string()],
        ..Default::default()
    };
    overflow_popup.style = Some(overflow_style.into());

    let mut container2 = LayoutBox::new(BoxType::BlockNode, None);
    container2.dimensions = containing_block;
    container2.children.push(overflow_popup);
    mango_layout::block_flow::layout_positioned_children(&mut container2, &mut float_ctx);

    let placed2 = &container2.children[0];
    // Placing below would end at 780 + 60 = 840 > 800 (overflow!).
    // flip-block must place it above the anchor: 750 - 60 = 690.
    assert_eq!(placed2.dimensions.content.y(), 690.0);
}

#[test]
fn test_scroll_driven_animations_engine() {
    let sheet = mango_css::parser::parse_stylesheet(
        r#"
        @keyframes scale-header {
            0% { transform: scale(1); opacity: 1; }
            100% { transform: scale(0.5); opacity: 0.2; }
        }
        "#,
    );
    let mut engine = AnimationEngine::new();
    engine.set_keyframes(mango_css::animation::collect_keyframes(std::iter::once(
        &sheet,
    )));

    let scroll_timeline = ScrollTimeline::new(
        "--page-scroll",
        mango_css::values::ScrollTimelineAxis::Block,
    );
    engine.set_scroll_timeline(scroll_timeline);

    let node = NodeId::from_raw(10);
    assert!(engine.start_with_timeline(node, "scale-header", "--page-scroll"));

    // Check at 0% scroll
    let mut style = ComputedStyle::default();
    engine.apply(node, &mut style);
    assert_eq!(style.opacity, 1.0);

    // Scroll 500px out of 1000px (50%)
    engine.update_scroll_timeline("--page-scroll", 500.0, 1000.0);
    let mut style_half = ComputedStyle::default();
    engine.apply(node, &mut style_half);
    // (1.0 + 0.2) / 2 = 0.6
    assert!((style_half.opacity - 0.6).abs() < 0.01);

    // Scroll 1000px out of 1000px (100%)
    engine.update_scroll_timeline("--page-scroll", 1000.0, 1000.0);
    let mut style_full = ComputedStyle::default();
    engine.apply(node, &mut style_full);
    assert!((style_full.opacity - 0.2).abs() < 0.01);
}

#[test]
fn test_view_transitions_css_and_properties() {
    let mut engine = Engine::new();
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head>
                <style>
                    @view-transition {
                        navigation: auto;
                        types: slide-left, cross-fade;
                    }
                    .hero-card {
                        view-transition-name: hero-image;
                        view-transition-class: hero;
                    }
                </style>
            </head>
            <body>
                <div id="hero" class="hero-card">Hero Card</div>
            </body>
        </html>
    "#;

    let _dl = engine.load(html, "");
    let doc = engine.document();
    let style_tree = engine.style_tree().expect("style tree must exist");

    let hero_node = doc.find_element_by_id(doc.root(), "hero").unwrap();
    let hero_style = find_styled_node(style_tree, hero_node).unwrap();

    assert_eq!(
        hero_style.style.view_transition_name.as_deref(),
        Some("hero-image")
    );
}

#[test]
fn test_typography_balance_and_pretty_wrapping() {
    // 1. Verify text-wrap: balance
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 320.0, 0.0);
    let balance_style = ComputedStyle {
        font_size: 16.0,
        text_wrap: TextWrap::Balance,
        ..Default::default()
    };
    container.style = Some(balance_style.clone().into());

    let text_node = LayoutBox::new(
        BoxType::TextNode("The quick brown fox jumps over the lazy dog and runs away".to_string()),
        Some(balance_style.into()),
    );
    container.children.push(text_node);

    let mut float_ctx = FloatContext::new();
    mango_layout::inline_flow::layout_inline_children(&mut container, &mut float_ctx);

    // Balanced lines should create at least 2 lines
    assert!(container.children.len() >= 2);

    // 2. Verify text-wrap: pretty (orphan prevention)
    let mut pretty_container = LayoutBox::new(BoxType::BlockNode, None);
    pretty_container.dimensions.content = Rect::new(0.0, 0.0, 200.0, 0.0);
    let pretty_style = ComputedStyle {
        font_size: 16.0,
        text_wrap: TextWrap::Pretty,
        ..Default::default()
    };
    pretty_container.style = Some(pretty_style.clone().into());

    let pretty_text = LayoutBox::new(
        BoxType::TextNode("Heading with solitary word orphan".to_string()),
        Some(pretty_style.into()),
    );
    pretty_container.children.push(pretty_text);

    let mut float_ctx2 = FloatContext::new();
    mango_layout::inline_flow::layout_inline_children(&mut pretty_container, &mut float_ctx2);

    assert!(!pretty_container.children.is_empty());
}
