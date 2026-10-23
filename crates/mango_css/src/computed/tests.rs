use super::*;
use mango_core::Color;
use mango_html::parse_html;

use crate::values::{BackgroundRepeat, BackgroundSize, BorderStyle, Length, Value};

#[test]
fn test_computed_style_inheritance() {
    let html = r#"<div style="color: red; font-size: 20px;"><p>Inherited</p></div>"#;
    let doc = parse_html(html);
    let root = doc.root();

    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    assert_eq!(div_style.color, Color::RED);
    assert_eq!(div_style.font_size, 20.0);

    // p should inherit color: red and font_size: 20px from div
    let p_style = compute_style(p_id, &doc, &[], Some(&div_style));
    assert_eq!(p_style.color, Color::RED);
    assert_eq!(p_style.font_size, 20.0);
    // But margin is not inherited (p gets UA margin, not div's)
    assert_ne!(p_style.margin_top, Length::Auto);
}

#[test]
fn test_computed_style_em_resolution() {
    let html = r#"<div style="font-size: 20px;"><h1 style="font-size: 2em;">Title</h1></div>"#;
    let doc = parse_html(html);
    let root = doc.root();

    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let h1_id = doc.find_element_by_tag(root, "h1").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    assert_eq!(div_style.font_size, 20.0);

    let h1_style = compute_style(h1_id, &doc, &[], Some(&div_style));
    // 2em * 20px = 40px
    assert_eq!(h1_style.font_size, 40.0);
}

#[test]
fn test_custom_property_basic_var_resolution() {
    let html = r#"<div style="--theme-color: #336699; --base-size: 24px;"><p style="color: var(--theme-color); font-size: var(--base-size);">Hello</p></div>"#;
    let doc = parse_html(html);
    let root = doc.root();

    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    let p_style = compute_style(p_id, &doc, &[], Some(&div_style));

    assert_eq!(p_style.color, Color::rgb(0x33, 0x66, 0x99));
    assert_eq!(p_style.font_size, 24.0);
}

#[test]
fn test_custom_property_inheritance_shares_storage_until_override() {
    let html = r#"
        <div style="--theme-color: #336699; --base-size: 24px">
            <p>shared</p>
            <p style="--theme-color: #ff0000">overridden</p>
        </div>
    "#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let paragraphs: Vec<_> = doc
        .children(div_id)
        .filter_map(|node| match node.data {
            mango_html::dom::NodeData::Element(ref el) if el.tag_name.eq_ignore_ascii_case("p") => {
                Some(node.id)
            }
            _ => None,
        })
        .collect();

    let parent = compute_style(div_id, &doc, &[], None);
    let shared = compute_style(paragraphs[0], &doc, &[], Some(&parent));
    let overridden = compute_style(paragraphs[1], &doc, &[], Some(&parent));

    assert!(std::sync::Arc::ptr_eq(
        &parent.custom_properties,
        &shared.custom_properties
    ));
    assert!(!std::sync::Arc::ptr_eq(
        &parent.custom_properties,
        &overridden.custom_properties
    ));
    assert_eq!(
        overridden.custom_properties.get("--theme-color"),
        Some(&Value::String("#ff0000".to_string()))
    );
}

#[test]
fn test_custom_property_fallback() {
    let html = r#"<p style="color: var(--non-existent, #00ff00); font-size: var(--missing-size, 18px);">Fallback</p>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    let p_style = compute_style(p_id, &doc, &[], None);
    assert_eq!(p_style.color, Color::rgb(0, 255, 0));
    assert_eq!(p_style.font_size, 18.0);
}

#[test]
fn test_custom_property_nested_var() {
    let html = r#"<div style="--level2: #ffa136;"><p style="color: var(--level1, var(--level2, #000000));">Nested</p></div>"#;
    let doc = parse_html(html);
    let root = doc.root();

    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    let p_style = compute_style(p_id, &doc, &[], Some(&div_style));

    assert_eq!(p_style.color, Color::rgb(255, 161, 54));
}

#[test]
fn test_custom_property_cycle_detection() {
    let html = r#"<p style="--a: var(--b); --b: var(--a); color: var(--a, #ff0000);">Cycle</p>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    let p_style = compute_style(p_id, &doc, &[], None);
    assert_eq!(p_style.color, Color::RED);
}

#[test]
fn test_custom_property_in_border_shorthand() {
    let html =
        r#"<div style="--border-color: #202122; border: 2px solid var(--border-color);">Box</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    assert_eq!(div_style.border_top_width, 2.0);
    assert_eq!(div_style.border_top_style, BorderStyle::Solid);
    assert_eq!(div_style.border_top_color, Color::rgb(0x20, 0x21, 0x22));
}

#[test]
fn test_custom_property_in_calc_expression() {
    let html =
        r#"<div style="margin-right: calc(var(--stack-gap-condensed, .5rem) * -1);">Spacer</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    let mr = div_style
        .margin_right
        .to_px_with_viewport(16.0, 16.0, 1000.0, 800.0);
    assert_eq!(mr, -8.0);

    let html_with_var = r#"<div style="--stack-gap-condensed: 12px; margin-right: calc(var(--stack-gap-condensed, .5rem) * -1);">Spacer</div>"#;
    let doc_var = parse_html(html_with_var);
    let root_var = doc_var.root();
    let div_id_var = doc_var.find_element_by_tag(root_var, "div").unwrap();

    let div_style_var = compute_style(div_id_var, &doc_var, &[], None);
    let mr_var = div_style_var
        .margin_right
        .to_px_with_viewport(16.0, 16.0, 1000.0, 800.0);
    assert_eq!(mr_var, -12.0);
}

#[test]
fn test_background_image_and_repeat_and_size() {
    let html = r#"<div style="background-image: url('logo.png'); background-repeat: no-repeat; background-size: cover;">Logo</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    assert_eq!(div_style.background_image.as_deref(), Some("logo.png"));
    assert_eq!(div_style.background_repeat, BackgroundRepeat::NoRepeat);
    assert_eq!(div_style.background_size, BackgroundSize::Cover);
}

#[test]
fn test_background_shorthand_expansion() {
    let html =
        r#"<div style="background: #ffffff url('/static/images/wiki.png') no-repeat;">Wiki</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    assert_eq!(div_style.background_color, Color::WHITE);
    assert_eq!(
        div_style.background_image.as_deref(),
        Some("/static/images/wiki.png")
    );
    assert_eq!(div_style.background_repeat, BackgroundRepeat::NoRepeat);
}

#[test]
fn test_letter_spacing_word_spacing_text_indent() {
    let html =
        r#"<div style="letter-spacing: 2px; word-spacing: 4px; text-indent: 20px;">Text</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    assert_eq!(div_style.letter_spacing, Length::Px(2.0));
    assert_eq!(div_style.word_spacing, Length::Px(4.0));
    assert_eq!(div_style.text_indent, Length::Px(20.0));
}

#[test]
fn test_spacing_and_indent_inheritance() {
    let html = r#"<div style="letter-spacing: 3px; word-spacing: 5px; text-indent: 1.5em;"><p>Child</p></div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    let p_style = compute_style(p_id, &doc, &[], Some(&div_style));
    assert_eq!(p_style.letter_spacing, Length::Px(3.0));
    assert_eq!(p_style.word_spacing, Length::Px(5.0));
    assert_eq!(p_style.text_indent, Length::Em(1.5));
}

#[test]
fn test_accent_color_and_current_color() {
    let html = r#"<div style="color: #ff5500; accent-color: currentcolor; background-color: currentcolor; border-top-color: currentcolor; outline-color: currentcolor;"><p style="accent-color: #00aaee;">Child</p></div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    assert_eq!(div_style.color, Color::rgb(255, 85, 0));
    assert_eq!(div_style.accent_color, Some(Color::rgb(255, 85, 0)));
    assert_eq!(div_style.background_color, Color::rgb(255, 85, 0));
    assert_eq!(div_style.border_top_color, Color::rgb(255, 85, 0));
    assert_eq!(div_style.outline_color, Color::rgb(255, 85, 0));

    let p_style = compute_style(p_id, &doc, &[], Some(&div_style));
    assert_eq!(p_style.accent_color, Some(Color::rgb(0, 170, 238)));

    // Test auto resets to None
    let auto_html = r#"<div style="accent-color: auto;">Auto</div>"#;
    let auto_doc = parse_html(auto_html);
    let auto_div = auto_doc
        .find_element_by_tag(auto_doc.root(), "div")
        .unwrap();
    let auto_style = compute_style(auto_div, &auto_doc, &[], None);
    assert_eq!(auto_style.accent_color, None);
}

#[test]
fn test_background_layers_deterministic_order() {
    let html = r#"<div style="background-image: url('a.png'), url('b.png'); background-repeat: repeat-x, no-repeat;">MultiBg</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    assert_eq!(div_style.background_layers.len(), 2);
    assert_eq!(
        div_style.background_layers[0].image.as_deref(),
        Some("a.png")
    );
    assert_eq!(
        div_style.background_layers[0].repeat,
        BackgroundRepeat::RepeatX
    );
    assert_eq!(
        div_style.background_layers[1].image.as_deref(),
        Some("b.png")
    );
    assert_eq!(
        div_style.background_layers[1].repeat,
        BackgroundRepeat::NoRepeat
    );
}

#[test]
fn test_background_size_and_position_parsing() {
    let html = r#"<div style="background-image: url('logo.svg'); background-size: 36px 36px; background-position: 50% 50%;">DDG Logo</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    assert_eq!(
        div_style.background_size,
        BackgroundSize::Explicit(Length::Px(36.0), Length::Px(36.0))
    );
    assert_eq!(
        div_style.background_position,
        (Length::Percent(50.0), Length::Percent(50.0))
    );
    assert_eq!(div_style.background_layers.len(), 1);
    assert_eq!(
        div_style.background_layers[0].size,
        BackgroundSize::Explicit(Length::Px(36.0), Length::Px(36.0))
    );
    assert_eq!(
        div_style.background_layers[0].position,
        (Length::Percent(50.0), Length::Percent(50.0))
    );
}

#[test]
fn test_percentage_line_height() {
    let html = r#"<div style="font-size: 20px; line-height: 150%;">Hello</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let style = compute_style(div_id, &doc, &[], None);
    assert_eq!(style.font_size, 20.0);
    assert_eq!(style.line_height, Some(30.0));
}

#[test]
fn test_border_width_without_style_defaults_none() {
    let html = r#"<div style="border-width: 5px;">No Border</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let style = compute_style(div_id, &doc, &[], None);
    // BorderStyle must remain None and computed border width must be zero per CSS 2.1 § 8.5.1
    assert_eq!(style.border_top_style, BorderStyle::None);
    assert_eq!(style.border_top_width, 0.0);
}

#[test]
fn test_border_radius_percentage() {
    let html = r#"<div style="border-radius: 50%;">Circle</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let style = compute_style(div_id, &doc, &[], None);
    assert_eq!(style.border_top_left_radius_len, Length::Percent(50.0));
    let radii = style.border_radius_with_size(100.0, 100.0);
    assert_eq!(radii, [50.0, 50.0, 50.0, 50.0]);
}

#[test]
fn test_text_shadow_multi_layers() {
    let html = r#"<div style="text-shadow: 1px 1px 2px black, 0 0 10px blue;">Glow</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let style = compute_style(div_id, &doc, &[], None);
    let shadows = style.all_text_shadows();
    assert_eq!(shadows.len(), 2);
    assert_eq!(shadows[0].offset_x, 1.0);
    assert_eq!(shadows[0].offset_y, 1.0);
    assert_eq!(shadows[0].blur_radius, 2.0);
    assert_eq!(shadows[1].offset_x, 0.0);
    assert_eq!(shadows[1].offset_y, 0.0);
    assert_eq!(shadows[1].blur_radius, 10.0);
}

#[test]
fn test_css_wide_keyword_inherit() {
    let html = r#"<div style="background-color: blue; opacity: 0.5;"><p style="background-color: inherit; opacity: inherit;">Child</p></div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    assert_eq!(div_style.background_color, Color::BLUE);
    assert_eq!(div_style.opacity, 0.5);

    let p_style = compute_style(p_id, &doc, &[], Some(&div_style));
    // background-color and opacity are NOT naturally inherited, but explicit 'inherit' forces them to inherit
    assert_eq!(p_style.background_color, Color::BLUE);
    assert_eq!(p_style.opacity, 0.5);
}

#[test]
fn test_css_wide_keyword_initial() {
    let html = r#"<div style="color: red; font-size: 32px;"><p style="color: initial; font-size: initial;">Child</p></div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    let p_style = compute_style(p_id, &doc, &[], Some(&div_style));
    // color and font-size would naturally inherit red/32px, but 'initial' resets them to initial values
    assert_eq!(p_style.color, Color::BLACK);
    assert_eq!(p_style.font_size, 16.0);
}

#[test]
fn test_css_wide_keyword_unset() {
    let html = r#"<div style="color: green; background-color: green;"><p style="color: unset; background-color: unset;">Child</p></div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    let p_style = compute_style(p_id, &doc, &[], Some(&div_style));
    // color is inherited -> unset acts as inherit (green)
    assert_eq!(p_style.color, Color::GREEN);
    // background-color is NOT inherited -> unset acts as initial (transparent)
    assert_eq!(p_style.background_color, Color::TRANSPARENT);
}

#[test]
fn test_css_wide_keyword_all_unset() {
    let html = r#"<div style="color: purple;"><button style="all: unset;">Button</button></div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let btn_id = doc.find_element_by_tag(root, "button").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    println!("div_style.color: {:?}", div_style.color);
    let btn_style = compute_style(btn_id, &doc, &[], Some(&div_style));
    println!("btn_style.color: {:?}", btn_style.color);
    assert_eq!(btn_style.color, Color::rgb(128, 0, 128));
}

#[test]
fn test_var_fallback_invalid_at_computed_value_time() {
    // Missing var without fallback becomes invalid at computed value time (unset)
    let html = r#"<div style="color: red;"><p style="color: var(--non-existent);">Child</p></div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    let div_style = compute_style(div_id, &doc, &[], None);
    let p_style = compute_style(p_id, &doc, &[], Some(&div_style));
    // color is inherited -> resolves as unset -> inherits parent's red
    assert_eq!(p_style.color, Color::RED);
}
