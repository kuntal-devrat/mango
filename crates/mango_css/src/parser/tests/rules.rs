use crate::parser::types::Rule;
use crate::parser::{CssParser, parse_stylesheet};
use crate::values::{
    Display, FlexDirection, FontStyle, FontWeight, GridPlacement, GridTrackSize, JustifyContent,
    Length, Value,
};
use mango_core::Color;

#[test]
fn test_parse_simple_stylesheet() {
    let css = r#"
        body {
            margin: 0px;
            background-color: #ffffff;
            color: black;
        }
        h1, h2 {
            font-weight: bold;
            font-size: 24px;
        }
    "#;

    let sheet = parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 2);

    if let Rule::Style(r1) = &sheet.rules[0] {
        assert_eq!(r1.selectors.selectors.len(), 1);
        assert!(
            r1.declarations
                .iter()
                .any(|d| d.name == "background-color" && d.value == Value::Color(Color::WHITE))
        );
    } else {
        panic!("expected StyleRule");
    }

    if let Rule::Style(r2) = &sheet.rules[1] {
        assert_eq!(r2.selectors.selectors.len(), 2); // h1 and h2
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_parse_important_declaration() {
    let css = "p { color: red !important; }";
    let sheet = parse_stylesheet(css);
    if let Rule::Style(r) = &sheet.rules[0] {
        assert_eq!(r.declarations.len(), 1);
        assert!(r.declarations[0].important);
        assert_eq!(r.declarations[0].value, Value::Color(Color::RED));
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_parse_viewport_units() {
    let css = "body { width: 60vw; margin: 15vh auto; }";
    let sheet = parse_stylesheet(css);
    if let Rule::Style(r) = &sheet.rules[0] {
        let width_decl = r.declarations.iter().find(|d| d.name == "width").unwrap();
        assert_eq!(width_decl.value, Value::Length(Length::Vw(60.0)));
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_parse_import_rules() {
    let css = r#"
        @import "base.css";
        @import url("theme.css");
        @import url('layout.css');
        @import url(typography.css);
        body { color: black; }
    "#;
    let sheet = parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 5);
    assert_eq!(sheet.rules[0], Rule::Import("base.css".to_string()));
    assert_eq!(sheet.rules[1], Rule::Import("theme.css".to_string()));
    assert_eq!(sheet.rules[2], Rule::Import("layout.css".to_string()));
    assert_eq!(sheet.rules[3], Rule::Import("typography.css".to_string()));
}

#[test]
fn test_parse_flex_declarations() {
    let css = r#"
        .nav {
            display: flex;
            flex-direction: row;
            justify-content: space-between;
            align-items: center;
            gap: 16px;
        }
        .item {
            flex: 1;
            order: 2;
            z-index: 10;
        }
    "#;
    let sheet = parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 2);
    if let Rule::Style(r1) = &sheet.rules[0] {
        assert!(
            r1.declarations
                .iter()
                .any(|d| d.name == "display" && d.value == Value::Display(Display::Flex))
        );
        assert!(
            r1.declarations.iter().any(|d| d.name == "flex-direction"
                && d.value == Value::FlexDirection(FlexDirection::Row))
        );
        assert!(r1.declarations.iter().any(|d| d.name == "justify-content"
            && d.value == Value::JustifyContent(JustifyContent::SpaceBetween)));
        assert!(
            r1.declarations
                .iter()
                .any(|d| d.name == "row-gap" && d.value == Value::Length(Length::Px(16.0)))
        );
        assert!(
            r1.declarations
                .iter()
                .any(|d| d.name == "column-gap" && d.value == Value::Length(Length::Px(16.0)))
        );
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_parse_media_rule_with_unsupported_tokens_does_not_loop() {
    let css = r#"
        @media (min-width: 600px) {
            @supports (display: grid) {
                .foo { display: grid; }
            }
            .bar { color: blue; }
            :::invalid_selector { margin: 10px; }
            .baz { color: red; }
        }
        body { background-color: white; }
    "#;
    let sheet = parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 2);
    if let Rule::Media(m) = &sheet.rules[0] {
        assert!(m.rules.iter().any(|r| {
            r.declarations
                .iter()
                .any(|d| d.name == "color" && d.value == Value::Color(Color::BLUE))
        }));
        assert!(m.rules.iter().any(|r| {
            r.declarations
                .iter()
                .any(|d| d.name == "color" && d.value == Value::Color(Color::RED))
        }));
    } else {
        panic!("expected MediaRule");
    }
}

#[test]
fn test_parse_grid_properties() {
    let css = r#"
        .container {
            display: grid;
            grid-template-columns: 200px 1fr;
            grid-template-rows: repeat(2, 50px);
        }
        .sidebar {
            grid-column: 1 / 2;
            grid-row: span 2;
        }
    "#;
    let sheet = parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 2);

    if let Rule::Style(r1) = &sheet.rules[0] {
        let cols = r1
            .declarations
            .iter()
            .find(|d| d.name == "grid-template-columns")
            .unwrap();
        assert_eq!(
            cols.value,
            Value::GridTrackList(vec![
                GridTrackSize::Length(Length::Px(200.0)),
                GridTrackSize::Fr(1.0),
            ])
        );
        let rows = r1
            .declarations
            .iter()
            .find(|d| d.name == "grid-template-rows")
            .unwrap();
        assert_eq!(
            rows.value,
            Value::GridTrackList(vec![
                GridTrackSize::Length(Length::Px(50.0)),
                GridTrackSize::Length(Length::Px(50.0)),
            ])
        );
    } else {
        panic!("expected StyleRule");
    }

    if let Rule::Style(r2) = &sheet.rules[1] {
        let col_start = r2
            .declarations
            .iter()
            .find(|d| d.name == "grid-column-start")
            .unwrap();
        let col_end = r2
            .declarations
            .iter()
            .find(|d| d.name == "grid-column-end")
            .unwrap();
        let row_start = r2
            .declarations
            .iter()
            .find(|d| d.name == "grid-row-start")
            .unwrap();

        assert_eq!(
            col_start.value,
            Value::GridPlacement(GridPlacement::Line(1))
        );
        assert_eq!(col_end.value, Value::GridPlacement(GridPlacement::Line(2)));
        assert_eq!(
            row_start.value,
            Value::GridPlacement(GridPlacement::Span(2))
        );
    } else {
        panic!("expected StyleRule");
    }

    let css_area = r#"
        .layout {
            grid-template: 50px 1fr / 200px 1fr;
            grid-template-areas: "header header" "sidebar content";
        }
        .header {
            grid-area: header;
        }
    "#;
    let sheet_area = parse_stylesheet(css_area);
    assert_eq!(sheet_area.rules.len(), 2);
    if let Rule::Style(r) = &sheet_area.rules[0] {
        let cols = r
            .declarations
            .iter()
            .find(|d| d.name == "grid-template-columns")
            .unwrap();
        let rows = r
            .declarations
            .iter()
            .find(|d| d.name == "grid-template-rows")
            .unwrap();
        assert_eq!(
            cols.value,
            Value::GridTrackList(vec![
                GridTrackSize::Length(Length::Px(200.0)),
                GridTrackSize::Fr(1.0),
            ])
        );
        assert_eq!(
            rows.value,
            Value::GridTrackList(vec![
                GridTrackSize::Length(Length::Px(50.0)),
                GridTrackSize::Fr(1.0),
            ])
        );
        let areas = r
            .declarations
            .iter()
            .find(|d| d.name == "grid-template-areas")
            .unwrap();
        assert_eq!(
            areas.value,
            Value::List(vec![
                Value::String("header header".to_string()),
                Value::String("sidebar content".to_string()),
            ])
        );
    } else {
        panic!("expected StyleRule");
    }
    if let Rule::Style(r) = &sheet_area.rules[1] {
        let row_start = r
            .declarations
            .iter()
            .find(|d| d.name == "grid-row-start")
            .unwrap();
        let col_start = r
            .declarations
            .iter()
            .find(|d| d.name == "grid-column-start")
            .unwrap();
        assert_eq!(
            row_start.value,
            Value::GridPlacement(GridPlacement::Area("header".to_string()))
        );
        assert_eq!(
            col_start.value,
            Value::GridPlacement(GridPlacement::Area("header".to_string()))
        );
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_parse_font_face_rule() {
    let css = r#"
        @font-face {
            font-family: 'Open Sans';
            src: url('fonts/OpenSans-Bold.woff2') format('woff2');
            font-weight: bold;
            font-style: italic;
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    match &sheet.rules[0] {
        Rule::FontFace(ff) => {
            assert_eq!(ff.font_family, "Open Sans");
            assert_eq!(ff.src_url, "fonts/OpenSans-Bold.woff2");
            assert_eq!(ff.font_weight, FontWeight::Bold);
            assert_eq!(ff.font_style, FontStyle::Italic);
        }
        _ => panic!("expected FontFace rule"),
    }
}

#[test]
fn test_parse_font_face_rule_with_multiple_src_prefers_woff2() {
    let css = r#"
        @font-face {
            font-family: WikipediaLocal;
            src: url(wiki.ttf), url(wiki.woff2) format('woff2'), url(wiki.woff);
            font-weight: 700;
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    match &sheet.rules[0] {
        Rule::FontFace(ff) => {
            assert_eq!(ff.font_family, "WikipediaLocal");
            assert_eq!(ff.src_url, "wiki.woff2");
            assert_eq!(ff.font_weight, FontWeight::Bold);
            assert_eq!(ff.font_style, FontStyle::Normal);
        }
        _ => panic!("expected FontFace rule"),
    }
}

#[test]
fn test_nested_supports_in_media() {
    let css = r#"
        @media screen {
            .vector-pinned-container { display: none; }
            @supports (display:grid) {
                .vector-pinned-container { display: block; }
            }
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    if let Rule::Media(mr) = &sheet.rules[0] {
        assert_eq!(mr.query, "screen");
        assert_eq!(mr.rules.len(), 2);
        let display_decls: Vec<&Value> = mr
            .rules
            .iter()
            .map(|r| {
                &r.declarations
                    .iter()
                    .find(|d| d.name == "display")
                    .unwrap()
                    .value
            })
            .collect();
        assert_eq!(
            display_decls,
            vec![
                &Value::Display(Display::None),
                &Value::Display(Display::Block)
            ]
        );
    } else {
        panic!("expected MediaRule");
    }
}

#[test]
fn test_font_face_formats_and_display() {
    let css = r#"
        @font-face {
            font-family: 'TestFont';
            src: url('font.otf') format('opentype'),
                 url('font.woff') format('woff'),
                 url('font.woff2') format('woff2'),
                 url('font.ttf') format('truetype');
            font-display: swap;
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    match &sheet.rules[0] {
        Rule::FontFace(ff) => {
            assert_eq!(ff.font_family, "TestFont");
            assert_eq!(
                ff.src_url, "font.woff2",
                "Must prefer woff2 over woff, ttf, otf"
            );
            assert_eq!(ff.font_display, crate::values::FontDisplay::Swap);
        }
        _ => panic!("expected FontFace rule"),
    }
}

#[test]
fn test_keyframes_invalid_selector_dropped() {
    let css = r#"
        @keyframes bounce {
            from { top: 0px; }
            unknown { top: 50px; }
            to { top: 100px; }
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    if let Rule::Keyframes(kf) = &sheet.rules[0] {
        assert_eq!(
            kf.keyframes.len(),
            2,
            "Only from and to should remain, invalid selector must be dropped"
        );
    } else {
        panic!("expected Keyframes");
    }
}

#[test]
fn test_attribute_selector_invalid_op_dropped() {
    let css = r#"
        [a~b] { color: red; }
        [lang|="en"] { color: blue; }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    // [a~b] is invalid and must not produce a rule
    assert_eq!(sheet.rules.len(), 1);
}

#[test]
fn test_supports_condition_evaluation() {
    let css = r#"
        @supports (display: grid) {
            .grid-supported { display: grid; }
        }
        @supports not (display: grid) {
            .grid-fallback { display: block; }
        }
        @supports (invalid-property-xyz: 123px) {
            .invalid-prop { color: red; }
        }
        @supports not (invalid-property-xyz: 123px) {
            .invalid-fallback { color: green; }
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 2);
    if let Rule::Media(m1) = &sheet.rules[0] {
        assert_eq!(m1.rules[0].declarations[0].name, "display");
        assert_eq!(
            m1.rules[0].declarations[0].value,
            Value::Display(crate::values::Display::Grid)
        );
    } else {
        panic!("expected media rule for grid-supported");
    }
    if let Rule::Media(m2) = &sheet.rules[1] {
        assert_eq!(m2.rules[0].declarations[0].name, "color");
        assert_eq!(
            m2.rules[0].declarations[0].value,
            Value::Color(Color::GREEN)
        );
    } else {
        panic!("expected media rule for invalid-fallback");
    }
}

#[test]
fn test_supports_color_revert_rule() {
    assert!(crate::supports("color:revert-rule", None));
    assert!(crate::supports("(color: revert-rule)", None));
    assert!(crate::supports("color", Some("revert-rule")));
}
