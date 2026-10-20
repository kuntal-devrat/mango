use crate::values::{
    ColorStop, Gradient, Length, MathExpr, TextTransform, TextWrap, Value, intern_math,
};
use mango_core::Color;

#[test]
fn test_text_transform() {
    assert_eq!(TextTransform::Uppercase.apply("hello world"), "HELLO WORLD");
    assert_eq!(TextTransform::Lowercase.apply("HELLO WORLD"), "hello world");
    assert_eq!(
        TextTransform::Capitalize.apply("hello world"),
        "Hello World"
    );
    assert_eq!(
        TextTransform::Capitalize.apply("foo-bar_baz"),
        "Foo-Bar_Baz"
    );
    assert_eq!(TextTransform::None.apply("Hello World"), "Hello World");
}

#[test]
fn test_length_to_px() {
    assert_eq!(Length::Px(24.0).to_px(16.0, 16.0, 1000.0), 24.0);
    assert_eq!(Length::Em(1.5).to_px(16.0, 16.0, 1000.0), 24.0);
    assert_eq!(Length::Rem(2.0).to_px(16.0, 14.0, 1000.0), 28.0);
    assert_eq!(Length::Percent(50.0).to_px(16.0, 16.0, 800.0), 400.0);
    assert!((Length::Vw(60.0).to_px(16.0, 16.0, 800.0) - 480.0).abs() < 0.001);
    assert_eq!(
        Length::Vh(15.0).to_px_with_viewport(16.0, 16.0, 800.0, 600.0),
        90.0
    );
    assert_eq!(
        Length::Vw(10.0).to_px_with_viewports(16.0, 16.0, 200.0, 800.0, 600.0),
        80.0
    );
    assert_eq!(
        Length::Vw(10.0).to_px_with_viewport(16.0, 16.0, 200.0, 600.0),
        80.0
    );
}

#[test]
fn test_parse_color_hex() {
    assert_eq!(Value::parse_color("#fff"), Some(Color::WHITE));
    assert_eq!(Value::parse_color("#000000"), Some(Color::BLACK));
    assert_eq!(Value::parse_color("#ff0000"), Some(Color::RED));
    assert_eq!(
        Value::parse_color("#ffa136"),
        Some(Color::rgb(255, 161, 54))
    );
}

#[test]
fn test_parse_color_rgb() {
    assert_eq!(Value::parse_color("rgb(255, 0, 0)"), Some(Color::RED));
    assert_eq!(
        Value::parse_color("rgba(0, 0, 0, 0)"),
        Some(Color::TRANSPARENT)
    );
    assert_eq!(Value::parse_color("rgb(255 0 0)"), Some(Color::RED));
    assert_eq!(
        Value::parse_color("rgb(255 0 0 / 0.5)"),
        Some(Color::rgba(255, 0, 0, 128))
    );
    assert_eq!(
        Value::parse_color("rgb(100% 0% 0% / 50%)"),
        Some(Color::rgba(255, 0, 0, 128))
    );
}

#[test]
fn test_named_colors() {
    assert_eq!(Value::parse_color("blue"), Some(Color::BLUE));
    assert_eq!(Value::parse_color("orange"), Some(Color::rgb(255, 165, 0)));
    assert_eq!(Value::parse_color("transparent"), Some(Color::TRANSPARENT));
    assert_eq!(
        Value::parse_color("cornflowerblue"),
        Some(Color::rgb(100, 149, 237))
    );
    assert_eq!(
        Value::parse_color("rebeccapurple"),
        Some(Color::rgb(102, 51, 153))
    );
    assert_eq!(
        Value::parse_color("gainsboro"),
        Some(Color::rgb(220, 220, 220))
    );
}

#[test]
fn test_parse_color_display_p3_and_srgb() {
    // sRGB
    assert_eq!(Value::parse_color("color(srgb 1 0 0)"), Some(Color::RED));
    assert_eq!(
        Value::parse_color("color(srgb 0 1 0 / 0.5)"),
        Some(Color::rgba(0, 255, 0, 128))
    );
    assert_eq!(
        Value::parse_color("color(srgb 100% 100% 0%)"),
        Some(Color::rgb(255, 255, 0))
    );

    // display-p3
    let p3_red = Value::parse_color("color(display-p3 1 0 0)").unwrap();
    assert_eq!(p3_red.r, 255);
    assert_eq!(p3_red.a, 255);

    let p3_white = Value::parse_color("color(display-p3 1 1 1 / 1)").unwrap();
    assert_eq!(p3_white, Color::WHITE);

    let p3_black = Value::parse_color("color(display-p3 0 0 0)").unwrap();
    assert_eq!(p3_black, Color::BLACK);
}

#[test]
fn test_parse_color_oklab_and_oklch() {
    // oklab
    assert_eq!(Value::parse_color("oklab(0 0 0)"), Some(Color::BLACK));
    assert_eq!(Value::parse_color("oklab(1 0 0)"), Some(Color::WHITE));
    let ok_red = Value::parse_color("oklab(0.628 0.225 0.126)").unwrap();
    assert!(ok_red.r >= 250);
    assert!(ok_red.g <= 10);
    assert!(ok_red.b <= 10);

    // oklch
    assert_eq!(Value::parse_color("oklch(0 0 0)"), Some(Color::BLACK));
    assert_eq!(Value::parse_color("oklch(100% 0 0deg)"), Some(Color::WHITE));
    let oklch_red = Value::parse_color("oklch(0.628 0.257 29.23deg)").unwrap();
    assert!(oklch_red.r >= 240);

    // oklch with turns and alpha
    let oklch_alpha = Value::parse_color("oklch(0.5 0.2 0.5turn / 50%)").unwrap();
    assert_eq!(oklch_alpha.a, 128);
}

#[test]
fn test_parse_color_mix() {
    // 50% / 50% in srgb
    let mixed = Value::parse_color("color-mix(in srgb, red, blue)").unwrap();
    assert!(mixed.r > 150);
    assert_eq!(mixed.g, 0);
    assert!(mixed.b > 150);
    assert_eq!(mixed.a, 255);

    // 100% red in srgb
    assert_eq!(
        Value::parse_color("color-mix(in srgb, red 100%, blue 0%)"),
        Some(Color::RED)
    );

    // color-mix in oklab
    let ok_mixed = Value::parse_color("color-mix(in oklab, red 50%, blue 50%)").unwrap();
    assert!(ok_mixed.r > 100);
    assert!(ok_mixed.b > 100);
    assert_eq!(ok_mixed.a, 255);

    // color-mix with alpha
    let alpha_mix = Value::parse_color(
        "color-mix(in srgb, rgba(255, 0, 0, 0.5) 50%, rgba(0, 0, 255, 0.5) 50%)",
    )
    .unwrap();
    assert_eq!(alpha_mix.a, 128);
}

#[test]
fn test_repeating_gradients_rasterize() {
    let grad = Gradient::Linear {
        angle_deg: 90.0,
        stops: vec![
            ColorStop::new(Color::RED, Some(0.0), None),
            ColorStop::new(Color::BLUE, Some(0.1), None),
        ],
        repeating: true,
    };
    let pixels = grad.rasterize(100, 10);
    assert_eq!(pixels.len(), 1000);
    // At every 10px period (10%), the pixel values repeat identically
    let c0 = pixels[0];
    let c10 = pixels[10];
    let c20 = pixels[20];
    let c30 = pixels[30];
    assert_eq!(c0, c10);
    assert_eq!(c10, c20);
    assert_eq!(c20, c30);
}

#[test]
fn test_colors_and_gradients() {
    // 1. color(display-p3)
    let p3_green = Value::parse_color("color(display-p3 0 1 0)").unwrap();
    assert!(p3_green.g > 240);

    // 2. oklab and oklch
    let oklab_blue = Value::parse_color("oklab(0.45 -0.03 -0.31)").unwrap();
    assert!(oklab_blue.b > 180);

    let oklch_green = Value::parse_color("oklch(0.86 0.29 142deg)").unwrap();
    assert!(oklch_green.g > 180);

    // 3. color-mix in oklch
    let mixed_lch = Value::parse_color("color-mix(in oklch, yellow 60%, green 40%)").unwrap();
    assert!(mixed_lch.g > 150);

    // 4. currentColor in gradient stops
    let mut grad = Gradient::Linear {
        angle_deg: 180.0,
        stops: vec![
            ColorStop {
                color: Color::BLACK,
                position: Some(0.0),
                end_position: None,
                is_current_color: true,
                var_name: None,
            },
            ColorStop::new(Color::WHITE, Some(1.0), None),
        ],
        repeating: false,
    };
    let custom_color = Color::rgb(255, 120, 0);
    grad.resolve_current_color(custom_color);
    assert_eq!(grad.stops()[0].color, custom_color);

    // 5. Conic gradient rasterization
    let conic = Gradient::Conic {
        angle_deg: 0.0,
        stops: vec![
            ColorStop::new(Color::RED, Some(0.0), None),
            ColorStop::new(Color::BLUE, Some(1.0), None),
        ],
        repeating: false,
    };
    let conic_pixels = conic.rasterize(50, 50);
    assert_eq!(conic_pixels.len(), 2500);
    assert!(conic_pixels.iter().any(|&p| p != 0));

    // 6. Radial gradient rasterization
    let radial = Gradient::Radial {
        stops: vec![
            ColorStop::new(Color::RED, Some(0.0), None),
            ColorStop::new(Color::BLUE, Some(1.0), None),
        ],
        repeating: false,
        center_x: 0.5,
        center_y: 0.5,
        radius_x: None,
        radius_y: None,
    };
    let radial_pixels = radial.rasterize(40, 40);
    assert_eq!(radial_pixels.len(), 1600);
    assert!(radial_pixels.iter().any(|&p| p != 0));
}

#[test]
fn test_intern_math_deduplication() {
    let expr1 = MathExpr::Add(
        Box::new(MathExpr::Length(Length::Px(10.0))),
        Box::new(MathExpr::Length(Length::Vw(5.0))),
    );
    let expr2 = expr1.clone();
    let id1 = intern_math(expr1);
    let id2 = intern_math(expr2);
    assert_eq!(id1, id2);
}

#[test]
fn test_length_parse_all_units() {
    assert!(matches!(Length::parse("72pt"), Some(Length::Px(px)) if (px - 96.0).abs() < 0.01));
    assert!(matches!(Length::parse("1in"), Some(Length::Px(px)) if (px - 96.0).abs() < 0.01));
    assert!(matches!(Length::parse("1pc"), Some(Length::Px(px)) if (px - 16.0).abs() < 0.01));
    assert!(matches!(Length::parse("2.54cm"), Some(Length::Px(px)) if (px - 96.0).abs() < 0.01));
    assert!(matches!(Length::parse("25.4mm"), Some(Length::Px(px)) if (px - 96.0).abs() < 0.01));
    assert!(matches!(Length::parse("101.6q"), Some(Length::Px(px)) if (px - 96.0).abs() < 0.01));
    assert!(matches!(Length::parse("2ch"), Some(Length::Em(em)) if (em - 1.0).abs() < 0.01));
    assert!(matches!(Length::parse("2ex"), Some(Length::Em(em)) if (em - 1.0).abs() < 0.01));
}

#[test]
fn test_relative_color_syntax_and_wide_gamut() {
    // 1. Wide gamut color spaces: rec2020, a98-rgb, prophoto-rgb
    let rec = Value::parse_color("color(rec2020 0.9 0.1 0.1)").unwrap();
    assert!(rec.r > 200);

    let a98 = Value::parse_color("color(a98-rgb 0.1 0.9 0.1)").unwrap();
    assert!(a98.g > 200);

    let pro = Value::parse_color("color(prophoto-rgb 0.1 0.1 0.9)").unwrap();
    assert!(pro.b > 200);

    // 2. Relative Color Syntax (RCS)
    // rgb from red
    let r_rcs = Value::parse_color("rgb(from red r g b)").unwrap();
    assert_eq!(r_rcs, Color::rgb(255, 0, 0));

    // rgb from with alpha
    let r_alpha = Value::parse_color("rgb(from #00ff00 r g b / 0.5)").unwrap();
    assert_eq!(r_alpha, Color::rgba(0, 255, 0, 128));

    // hsl from blue
    let hsl_rcs = Value::parse_color("hsl(from blue h s l)").unwrap();
    assert_eq!(hsl_rcs, Color::rgb(0, 0, 255));

    // oklch from green
    let oklch_rcs = Value::parse_color("oklch(from #00ff00 l c h)").unwrap();
    assert!(oklch_rcs.g > 200);

    // oklab from red
    let oklab_rcs = Value::parse_color("oklab(from red l a b)").unwrap();
    assert!(oklab_rcs.r > 200);

    // 3. TextWrap parsing
    assert_eq!(TextWrap::parse("balance"), Some(TextWrap::Balance));
    assert_eq!(TextWrap::parse("pretty"), Some(TextWrap::Pretty));
    assert_eq!(TextWrap::parse("nowrap"), Some(TextWrap::Nowrap));
    assert_eq!(TextWrap::parse("wrap"), Some(TextWrap::Wrap));
}
