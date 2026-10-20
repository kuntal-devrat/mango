use crate::parser::CssParser;
use crate::parser::types::Rule;
use crate::values::{Length, Value};

#[test]
fn test_parse_calc_function() {
    let css = r#"
        .container {
            width: calc(100% - 20px);
            max-width: calc(50vw + 10px);
            margin-left: calc(2rem - 4px);
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    if let Rule::Style(sr) = &sheet.rules[0] {
        let width = sr.declarations.iter().find(|d| d.name == "width").unwrap();
        match &width.value {
            Value::Length(Length::Calc(c)) => {
                assert_eq!(c.percent, 100.0);
                assert_eq!(c.px, -20.0);
            }
            other => panic!("expected calc length, got {:?}", other),
        }

        let max_w = sr
            .declarations
            .iter()
            .find(|d| d.name == "max-width")
            .unwrap();
        match &max_w.value {
            Value::Length(Length::Calc(c)) => {
                assert_eq!(c.vw, 50.0);
                assert_eq!(c.px, 10.0);
            }
            other => panic!("expected calc length, got {:?}", other),
        }

        let ml = sr
            .declarations
            .iter()
            .find(|d| d.name == "margin-left")
            .unwrap();
        match &ml.value {
            Value::Length(Length::Calc(c)) => {
                assert_eq!(c.rem, 2.0);
                assert_eq!(c.px, -4.0);
            }
            other => panic!("expected calc length, got {:?}", other),
        }
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_parse_box_shadow() {
    let css = r#"
        .card {
            box-shadow: 2px 4px 8px 1px rgba(0, 0, 0, 0.2);
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    if let Rule::Style(sr) = &sheet.rules[0] {
        let shadow = sr
            .declarations
            .iter()
            .find(|d| d.name == "box-shadow")
            .unwrap();
        match &shadow.value {
            Value::BoxShadow(bs) => {
                assert_eq!(bs.offset_x, 2.0);
                assert_eq!(bs.offset_y, 4.0);
                assert_eq!(bs.blur_radius, 8.0);
                assert_eq!(bs.spread_radius, 1.0);
                assert!(!bs.inset);
            }
            other => panic!("expected BoxShadow, got {:?}", other),
        }
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_parse_multiple_box_shadow() {
    let css = r#"
        .card {
            box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12), 0 1px 2px rgba(0, 0, 0, 0.24);
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    if let Rule::Style(sr) = &sheet.rules[0] {
        let shadow = sr
            .declarations
            .iter()
            .find(|d| d.name == "box-shadow")
            .unwrap();
        match &shadow.value {
            Value::List(items) => {
                assert_eq!(items.len(), 2);
                match (&items[0], &items[1]) {
                    (Value::BoxShadow(s1), Value::BoxShadow(s2)) => {
                        assert_eq!(s1.offset_x, 0.0);
                        assert_eq!(s1.offset_y, 1.0);
                        assert_eq!(s1.blur_radius, 3.0);
                        assert_eq!(s2.offset_x, 0.0);
                        assert_eq!(s2.offset_y, 1.0);
                        assert_eq!(s2.blur_radius, 2.0);
                    }
                    _ => panic!("expected Value::BoxShadow items"),
                }
            }
            other => panic!("expected Value::List, got {:?}", other),
        }
    }
}

#[test]
fn test_font_variation_and_feature_settings() {
    let css = r#"
        .features {
            font-variation-settings: "wght" 750, "wdth" 85.5;
            font-feature-settings: "liga" 1, "smcp" on, "swsh" 2, "dlig" off;
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    if let Rule::Style(sr) = &sheet.rules[0] {
        let fvs = sr
            .declarations
            .iter()
            .find(|d| d.name == "font-variation-settings")
            .unwrap();
        match &fvs.value {
            Value::FontVariationSettings(crate::values::FontVariationSettings::Settings(s)) => {
                assert_eq!(s.len(), 2);
                assert_eq!(s[0], ("wght".to_string(), 750.0));
                assert_eq!(s[1], ("wdth".to_string(), 85.5));
            }
            _ => panic!("expected FontVariationSettings::Settings"),
        }

        let ffs = sr
            .declarations
            .iter()
            .find(|d| d.name == "font-feature-settings")
            .unwrap();
        match &ffs.value {
            Value::FontFeatureSettings(crate::values::FontFeatureSettings::Features(f)) => {
                assert_eq!(f.len(), 4);
                assert_eq!(f[0], ("liga".to_string(), 1));
                assert_eq!(f[1], ("smcp".to_string(), 1));
                assert_eq!(f[2], ("swsh".to_string(), 2));
                assert_eq!(f[3], ("dlig".to_string(), 0));
            }
            _ => panic!("expected FontFeatureSettings::Features"),
        }
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_wrapping_and_hyphenation_properties() {
    let css = r#"
        .text-rules {
            word-break: break-all;
            overflow-wrap: anywhere;
            hyphens: auto;
            line-clamp: 3;
            -webkit-line-clamp: 2;
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    if let Rule::Style(sr) = &sheet.rules[0] {
        let wb = sr
            .declarations
            .iter()
            .find(|d| d.name == "word-break")
            .unwrap();
        assert_eq!(
            wb.value,
            Value::WordBreak(crate::values::WordBreak::BreakAll)
        );

        let ow = sr
            .declarations
            .iter()
            .find(|d| d.name == "overflow-wrap")
            .unwrap();
        assert_eq!(
            ow.value,
            Value::OverflowWrap(crate::values::OverflowWrap::Anywhere)
        );

        let hyp = sr
            .declarations
            .iter()
            .find(|d| d.name == "hyphens")
            .unwrap();
        assert_eq!(hyp.value, Value::Hyphens(crate::values::Hyphens::Auto));

        let lc = sr
            .declarations
            .iter()
            .find(|d| d.name == "line-clamp")
            .unwrap();
        assert_eq!(
            lc.value,
            Value::LineClamp(crate::values::LineClamp::Lines(3))
        );

        let wlc = sr
            .declarations
            .iter()
            .find(|d| d.name == "-webkit-line-clamp")
            .unwrap();
        assert_eq!(
            wlc.value,
            Value::LineClamp(crate::values::LineClamp::Lines(2))
        );
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_text_decorations_and_emphasis() {
    let css = r#"
        .decorations {
            text-underline-offset: 4px;
            text-decoration-thickness: 2px;
            text-emphasis: filled circle #ff0000;
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    if let Rule::Style(sr) = &sheet.rules[0] {
        let tuo = sr
            .declarations
            .iter()
            .find(|d| d.name == "text-underline-offset")
            .unwrap();
        assert_eq!(tuo.value, Value::Length(crate::values::Length::Px(4.0)));

        let tdt = sr
            .declarations
            .iter()
            .find(|d| d.name == "text-decoration-thickness")
            .unwrap();
        assert_eq!(
            tdt.value,
            Value::TextDecorationThickness(crate::values::TextDecorationThickness::Length(
                crate::values::Length::Px(2.0)
            ))
        );

        let tes = sr
            .declarations
            .iter()
            .find(|d| d.name == "text-emphasis-style")
            .unwrap();
        assert_eq!(
            tes.value,
            Value::TextEmphasisStyle(crate::values::TextEmphasisStyle::FilledCircle)
        );

        let tec = sr
            .declarations
            .iter()
            .find(|d| d.name == "text-emphasis-color")
            .unwrap();
        assert_eq!(tec.value, Value::Color(mango_core::Color::rgb(255, 0, 0)));
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_important_with_trailing_tokens_dropped() {
    let css = r#"
        div {
            color: red !important blue;
            background-color: green !important;
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    if let Rule::Style(sr) = &sheet.rules[0] {
        assert!(
            sr.declarations.iter().all(|d| d.name != "color"),
            "color with tokens after !important must be dropped"
        );
        assert!(
            sr.declarations
                .iter()
                .any(|d| d.name == "background-color" && d.important)
        );
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_unknown_unit_declaration_dropped() {
    let css = r#"
        div {
            width: 100blorp;
            height: 200px;
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    if let Rule::Style(sr) = &sheet.rules[0] {
        assert_eq!(
            sr.declarations.len(),
            1,
            "100blorp must be rejected per CSS spec"
        );
        assert_eq!(sr.declarations[0].name, "height");
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_parse_min_max_clamp_standalone() {
    let css = r#"
        .box {
            width: min(100px, 200px);
            height: max(50px, 150px);
            top: clamp(10px, 25px, 20px);
            bottom: clamp(10px, 5px, 20px);
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    if let Rule::Style(sr) = &sheet.rules[0] {
        let width = sr.declarations.iter().find(|d| d.name == "width").unwrap();
        assert_eq!(width.value, Value::Length(Length::Px(100.0)));

        let height = sr.declarations.iter().find(|d| d.name == "height").unwrap();
        assert_eq!(height.value, Value::Length(Length::Px(150.0)));

        let top = sr.declarations.iter().find(|d| d.name == "top").unwrap();
        assert_eq!(top.value, Value::Length(Length::Px(20.0)));

        let bottom = sr.declarations.iter().find(|d| d.name == "bottom").unwrap();
        assert_eq!(bottom.value, Value::Length(Length::Px(10.0)));
    } else {
        panic!("expected StyleRule");
    }
}

#[test]
fn test_resolve_clamp_dynamic() {
    let css = r#"
        .box {
            width: clamp(100px, 50vw, 400px);
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    if let Rule::Style(sr) = &sheet.rules[0] {
        let width = sr.declarations.iter().find(|d| d.name == "width").unwrap();
        if let Value::Length(len) = &width.value {
            // Viewport 1000px -> 50vw = 500px -> clamp(100, 500, 400) = 400px
            let px_1000 = len.to_px_with_viewports(16.0, 16.0, 800.0, 1000.0, 600.0);
            assert_eq!(px_1000, 400.0);

            // Viewport 600px -> 50vw = 300px -> clamp(100, 300, 400) = 300px
            let px_600 = len.to_px_with_viewports(16.0, 16.0, 800.0, 600.0, 600.0);
            assert_eq!(px_600, 300.0);

            // Viewport 100px -> 50vw = 50px -> clamp(100, 50, 400) = 100px
            let px_100 = len.to_px_with_viewports(16.0, 16.0, 800.0, 100.0, 600.0);
            assert_eq!(px_100, 100.0);
        } else {
            panic!("expected Value::Length");
        }
    }
}

#[test]
fn test_nested_calc_and_min() {
    let css = r#"
        .box {
            width: calc(min(100px, 50%) + 10px);
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    if let Rule::Style(sr) = &sheet.rules[0] {
        let width = sr.declarations.iter().find(|d| d.name == "width").unwrap();
        if let Value::Length(len) = &width.value {
            // Container 400px -> 50% = 200px -> min(100, 200) = 100 -> + 10 = 110px
            let px_400 = len.to_px_with_viewports(16.0, 16.0, 400.0, 800.0, 600.0);
            assert_eq!(px_400, 110.0);

            // Container 100px -> 50% = 50px -> min(100, 50) = 50 -> + 10 = 60px
            let px_100 = len.to_px_with_viewports(16.0, 16.0, 100.0, 800.0, 600.0);
            assert_eq!(px_100, 60.0);
        } else {
            panic!("expected Value::Length");
        }
    }
}

#[test]
fn test_length_parse_math_strings() {
    let len1 = Length::parse("min(10px, 20px)").expect("min parse");
    assert_eq!(len1, Length::Px(10.0));

    let len2 = Length::parse("clamp(100px, 50vw, 400px)").expect("clamp parse");
    let px = len2.to_px_with_viewports(16.0, 16.0, 800.0, 1000.0, 600.0);
    assert_eq!(px, 400.0);
}

#[test]
fn test_transition_longhands_and_cubic_bezier() {
    let css = r#"
        .animated {
            transition-property: opacity, transform;
            transition-duration: 300ms;
            transition-timing-function: cubic-bezier(0.25, 0.1, 0.25, 1.0);
            transition-delay: 50ms;
        }
    "#;
    let sheet = CssParser::parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    if let Rule::Style(sr) = &sheet.rules[0] {
        let decl_prop = sr
            .declarations
            .iter()
            .find(|d| d.name == "transition-property")
            .unwrap();
        assert_eq!(decl_prop.name, "transition-property");

        let decl_dur = sr
            .declarations
            .iter()
            .find(|d| d.name == "transition-duration")
            .unwrap();
        assert_eq!(decl_dur.value, Value::Time(300.0));

        let decl_tf = sr
            .declarations
            .iter()
            .find(|d| d.name == "transition-timing-function")
            .unwrap();
        assert_eq!(
            decl_tf.value,
            Value::TimingFunction(crate::values::TimingFunction::CubicBezier(
                0.25, 0.1, 0.25, 1.0
            ))
        );

        let decl_del = sr
            .declarations
            .iter()
            .find(|d| d.name == "transition-delay")
            .unwrap();
        assert_eq!(decl_del.value, Value::Time(50.0));
    } else {
        panic!("expected style rule");
    }
}
