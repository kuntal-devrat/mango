use std::collections::HashMap;
use mango_css::parser::{parse_declaration_list, parse_stylesheet, Rule};
use mango_css::properties::Declaration;
use mango_css::values::{
    BlendMode, ClipPath, FilterFunction, Length, MaskMode, TransformFunction, Value,
};
use mango_css::computed::ComputedStyle;

fn apply_props(style: &mut ComputedStyle, decls: &[Declaration]) {
    let mut map = HashMap::new();
    for d in decls {
        map.insert(d.name.clone(), d.value.clone());
    }
    mango_css::computed::apply_cascaded_properties(style, &map, None);
}

fn get_style_decls(rule: &Rule) -> &[Declaration] {
    match rule {
        Rule::Style(s) => &s.declarations,
        _ => panic!("expected style rule"),
    }
}

#[test]
fn test_2d_transforms() {
    let css = r#"
        .a { transform: translate(10px, 20px) rotate(45deg) scale(1.5, 2.0) skew(10deg, 5deg) matrix(1, 0, 0, 1, 5, 5); }
    "#;
    let sheet = parse_stylesheet(css);
    let decls = get_style_decls(&sheet.rules[0]);
    let transform_decl = decls.iter().find(|d| d.name == "transform").unwrap();
    if let Value::Transform(t) = &transform_decl.value {
        assert_eq!(t.0.len(), 5);
        assert!(matches!(t.0[0], TransformFunction::Translate(10.0, 20.0)));
        assert!(matches!(t.0[1], TransformFunction::Rotate(r) if (r - 45.0).abs() < 1e-4));
        assert!(matches!(t.0[2], TransformFunction::Scale(1.5, 2.0)));
        assert!(matches!(t.0[3], TransformFunction::Skew(10.0, 5.0)));
        assert!(matches!(t.0[4], TransformFunction::Matrix(1.0, 0.0, 0.0, 1.0, 5.0, 5.0)));
    } else {
        panic!("expected Value::Transform");
    }
}

#[test]
fn test_3d_transforms() {
    let css = r#"
        .b { transform: translate3d(10px, 20px, 30px) translateZ(40px) rotateX(30deg) rotateY(45deg) rotateZ(60deg) rotate3d(1, 0, 0, 90deg) scale3d(2, 3, 4) scaleZ(5) perspective(500px); }
    "#;
    let sheet = parse_stylesheet(css);
    let decls = get_style_decls(&sheet.rules[0]);
    let transform_decl = decls.iter().find(|d| d.name == "transform").unwrap();
    if let Value::Transform(t) = &transform_decl.value {
        assert_eq!(t.0.len(), 9);
        assert!(matches!(t.0[0], TransformFunction::Translate3d(10.0, 20.0, 30.0)));
        assert!(matches!(t.0[1], TransformFunction::TranslateZ(40.0)));
        assert!(matches!(t.0[2], TransformFunction::RotateX(30.0)));
        assert!(matches!(t.0[3], TransformFunction::RotateY(45.0)));
        assert!(matches!(t.0[4], TransformFunction::RotateZ(60.0)));
        assert!(matches!(t.0[5], TransformFunction::Rotate3d(1.0, 0.0, 0.0, 90.0)));
        assert!(matches!(t.0[6], TransformFunction::Scale3d(2.0, 3.0, 4.0)));
        assert!(matches!(t.0[7], TransformFunction::ScaleZ(5.0)));
        assert!(matches!(t.0[8], TransformFunction::Perspective(500.0)));

        // Test projection to 2D matrix
        let mat = t.to_matrix();
        assert!(!mat.iter().any(|v| v.is_nan()));
    } else {
        panic!("expected Value::Transform");
    }
}

#[test]
fn test_transform_origin_variations() {
    let mut style = ComputedStyle::default();
    
    // 1 keyword: center
    let decls = parse_declaration_list("transform-origin: center;");
    apply_props(&mut style, &decls);
    assert_eq!(style.transform_origin_x, Length::Percent(50.0));
    assert_eq!(style.transform_origin_y, Length::Percent(50.0));
    assert_eq!(style.transform_origin_z, Length::Px(0.0));

    // 2 keywords reversed: top left
    let decls = parse_declaration_list("transform-origin: top left;");
    apply_props(&mut style, &decls);
    assert_eq!(style.transform_origin_x, Length::Percent(0.0));
    assert_eq!(style.transform_origin_y, Length::Percent(0.0));

    // 3 values with Z-offset: 20px 30px 50px
    let decls = parse_declaration_list("transform-origin: 20px 30px 50px;");
    apply_props(&mut style, &decls);
    assert_eq!(style.transform_origin_x, Length::Px(20.0));
    assert_eq!(style.transform_origin_y, Length::Px(30.0));
    assert_eq!(style.transform_origin_z, Length::Px(50.0));

    // Longhands: transform-origin-z
    let decls = parse_declaration_list("transform-origin-z: 100px;");
    apply_props(&mut style, &decls);
    assert_eq!(style.transform_origin_z, Length::Px(100.0));
}

#[test]
fn test_transitions_and_animations() {
    let css = r#"
        .box {
            transition: opacity 250ms ease-in-out 50ms, transform 500ms cubic-bezier(0.25, 0.1, 0.25, 1.0);
            animation: bounce 1s ease-in 100ms 3 alternate forwards running;
        }
    "#;
    let sheet = parse_stylesheet(css);
    let decls = get_style_decls(&sheet.rules[0]);
    assert!(decls.iter().any(|d| d.name == "transition-property"));
    assert!(decls.iter().any(|d| d.name == "transition-duration"));
    assert!(decls.iter().any(|d| d.name == "transition-timing-function"));
    assert!(decls.iter().any(|d| d.name == "transition-delay"));
    assert!(decls.iter().any(|d| d.name == "animation-name"));
    assert!(decls.iter().any(|d| d.name == "animation-duration"));
    assert!(decls.iter().any(|d| d.name == "animation-iteration-count"));
    assert!(decls.iter().any(|d| d.name == "animation-direction"));
    assert!(decls.iter().any(|d| d.name == "animation-fill-mode"));
    assert!(decls.iter().any(|d| d.name == "animation-play-state"));
}

#[test]
fn test_keyframes_parsing() {
    let css = r#"
        @keyframes slide {
            from { transform: translateX(0px); opacity: 0; }
            50% { opacity: 0.5; }
            to { transform: translateX(100px); opacity: 1; }
        }
    "#;
    let sheet = parse_stylesheet(css);
    let kf_rule = match &sheet.rules[0] {
        Rule::Keyframes(k) => k,
        _ => panic!("expected keyframes rule"),
    };
    assert_eq!(kf_rule.name, "slide");
    assert_eq!(kf_rule.keyframes.len(), 3);
    assert_eq!(kf_rule.keyframes[0].offsets, vec![0.0]);
    assert_eq!(kf_rule.keyframes[1].offsets, vec![50.0]);
    assert_eq!(kf_rule.keyframes[2].offsets, vec![100.0]);
}

#[test]
fn test_all_10_filters_and_backdrop_filter() {
    let css = r#"
        .f {
            filter: blur(5px) brightness(1.2) contrast(150%) drop-shadow(4px 4px 10px black) grayscale(80%) hue-rotate(90deg) invert(100%) opacity(0.8) saturate(200%) sepia(30%);
            backdrop-filter: blur(10px) brightness(0.9);
        }
    "#;
    let sheet = parse_stylesheet(css);
    let decls = get_style_decls(&sheet.rules[0]);
    let filter_decl = decls.iter().find(|d| d.name == "filter").unwrap();
    if let Value::Filter(fns) = &filter_decl.value {
        assert_eq!(fns.len(), 10);
        assert!(matches!(fns[0], FilterFunction::Blur(5.0)));
        assert!(matches!(fns[1], FilterFunction::Brightness(1.2)));
        assert!(matches!(fns[2], FilterFunction::Contrast(1.5)));
        assert!(matches!(fns[3], FilterFunction::DropShadow { .. }));
        assert!(matches!(fns[4], FilterFunction::Grayscale(0.8)));
        assert!(matches!(fns[5], FilterFunction::HueRotate(90.0)));
        assert!(matches!(fns[6], FilterFunction::Invert(1.0)));
        assert!(matches!(fns[7], FilterFunction::Opacity(0.8)));
        assert!(matches!(fns[8], FilterFunction::Saturate(2.0)));
        assert!(matches!(fns[9], FilterFunction::Sepia(0.3)));
    } else {
        panic!("expected Value::Filter");
    }

    let backdrop_decl = decls.iter().find(|d| d.name == "backdrop-filter").unwrap();
    if let Value::Filter(fns) = &backdrop_decl.value {
        assert_eq!(fns.len(), 2);
        assert!(matches!(fns[0], FilterFunction::Blur(10.0)));
        assert!(matches!(fns[1], FilterFunction::Brightness(0.9)));
    } else {
        panic!("expected backdrop-filter to parse as Value::Filter");
    }
}

#[test]
fn test_blend_modes() {
    let modes = [
        ("normal", BlendMode::Normal),
        ("multiply", BlendMode::Multiply),
        ("screen", BlendMode::Screen),
        ("overlay", BlendMode::Overlay),
        ("darken", BlendMode::Darken),
        ("lighten", BlendMode::Lighten),
        ("color-dodge", BlendMode::ColorDodge),
        ("color-burn", BlendMode::ColorBurn),
        ("hard-light", BlendMode::HardLight),
        ("soft-light", BlendMode::SoftLight),
        ("difference", BlendMode::Difference),
        ("exclusion", BlendMode::Exclusion),
        ("hue", BlendMode::Hue),
        ("saturation", BlendMode::Saturation),
        ("color", BlendMode::Color),
        ("luminosity", BlendMode::Luminosity),
    ];

    for (name, expected) in modes {
        let css = format!("mix-blend-mode: {}; background-blend-mode: {};", name, name);
        let decls = parse_declaration_list(&css);
        let mut style = ComputedStyle::default();
        apply_props(&mut style, &decls);
        assert_eq!(style.mix_blend_mode, expected, "Failed for mix-blend-mode: {}", name);
        assert_eq!(style.background_blend_mode, expected, "Failed for background-blend-mode: {}", name);
    }
}

#[test]
fn test_clip_paths() {
    let decls = parse_declaration_list(r#"
        clip-path: polygon(0% 0%, 100% 0%, 50% 100%);
    "#);
    let mut style = ComputedStyle::default();
    apply_props(&mut style, &decls);
    match &style.clip_path {
        ClipPath::Polygon(pts) => {
            assert_eq!(pts.len(), 3);
            assert_eq!(pts[0], (Length::Percent(0.0), Length::Percent(0.0)));
            assert_eq!(pts[1], (Length::Percent(100.0), Length::Percent(0.0)));
            assert_eq!(pts[2], (Length::Percent(50.0), Length::Percent(100.0)));
        }
        other => panic!("expected polygon, got {:?}", other),
    }

    let decls = parse_declaration_list("clip-path: circle(50px at center);");
    apply_props(&mut style, &decls);
    assert!(matches!(style.clip_path, ClipPath::Circle { radius: Length::Px(50.0), .. }));

    let decls = parse_declaration_list("clip-path: inset(10px 20px 30px 40px round 5px);");
    apply_props(&mut style, &decls);
    assert!(matches!(style.clip_path, ClipPath::Inset { round: Some(_), .. }));
}

#[test]
fn test_mask_and_will_change() {
    let decl = Declaration::new(
        "mask",
        Value::List(vec![
            Value::Url("icon.svg".to_string()),
            Value::Keyword("luminance".to_string()),
        ]),
        false,
    );
    let expanded = decl.expand_shorthand();
    assert_eq!(expanded[0].name, "mask-image");
    assert_eq!(expanded[0].value, Value::Url("icon.svg".to_string()));
    assert_eq!(expanded[1].name, "mask-mode");
    assert_eq!(expanded[1].value, Value::MaskMode(MaskMode::Luminance));

    let decls = parse_declaration_list("will-change: transform, opacity, filter;");
    let mut style = ComputedStyle::default();
    apply_props(&mut style, &decls);
    assert!(style.will_change);
    assert_eq!(style.will_change_properties, vec!["transform", "opacity", "filter"]);
}
