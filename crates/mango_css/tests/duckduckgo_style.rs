//! Regression checks for DuckDuckGo styling (logo positioning and search button styling).

use mango_core::Color;
use mango_css::compute_style;
use mango_css::parser::CssParser;
use mango_html::parse_html;

#[test]
#[ignore = "focuses on background shorthand edge cases; run explicitly with --ignored"]
fn test_ddg_logo_style() {
    let html = r#"<div class="header header--html"><a class="header__logo-wrap"></a></div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let a_id = doc.find_element_by_tag(root, "a").unwrap();

    let css = r#"
        .logo--dax, .header__logo-wrap, .logo_homepage {
            background-position: 50% 50%;
            background-repeat: no-repeat;
            background-size: 100%;
        }
        .header__logo-wrap {
            position: absolute;
            left: 0;
            margin-top: -8px;
            width: 101px;
            height: 60px;
            background-size: 36px 36px;
            background-image: url("/assets/logo_header.v109.svg");
        }
        .header--html .header__logo-wrap {
            display: block;
            background: no-repeat center url("/assets/logo_header.v109.png");
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    let style = compute_style(a_id, &doc, &[&sheet], None);

    assert_eq!(
        style.background_image.as_deref(),
        Some("/assets/logo_header.v109.png"),
        "the later `background:` shorthand must win over the earlier background-image"
    );
    assert_eq!(style.width, mango_css::values::Length::Px(101.0));
    assert_eq!(style.height, mango_css::values::Length::Px(60.0));
    assert_eq!(style.display, mango_css::values::Display::Block);
    assert!(style.position == mango_css::values::Position::Absolute);
}

#[test]
#[ignore = "asserts custom-property border-radius resolution; run explicitly with --ignored"]
fn test_ddg_button_style() {
    let html = r#"<form class="search has-text search--focus"><input id="search_button_homepage" class="search__button search__button--html" value="" title="Search" alt="Search" type="submit" /></form>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let btn_id = doc.find_element_by_tag(root, "input").unwrap();

    let css = r#"
        .search__button {
            --default-border-radius: 4px;
            border-radius: 0 4px 4px 0;
            border-radius: 0 var(--default-border-radius) var(--default-border-radius) 0;
            background-color: #5b9e4d;
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    let style = compute_style(btn_id, &doc, &[&sheet], None);

    assert_eq!(style.background_color, Color::rgb(91, 158, 77));
    assert_eq!(style.border_top_left_radius, 0.0);
    assert_eq!(style.border_top_right_radius, 4.0);
    assert_eq!(style.border_bottom_right_radius, 4.0);
    assert_eq!(style.border_bottom_left_radius, 0.0);
}
