use mango_core::Color;
use mango_html::parse_html;

use crate::cascade::media::*;
use crate::cascade::resolver::resolve_cascade;
use crate::parser::parse_stylesheet;
use crate::values::{Length, Value};

#[test]
fn test_media_query_calc() {
    assert!(!matches_media_query_size(
        "screen and (max-width:calc(1120px - 1px))",
        1280.0,
        900.0
    ));
    assert!(matches_media_query_size(
        "screen and (max-width:calc(1120px - 1px))",
        1000.0,
        900.0
    ));
    assert!(matches_media_query_size(
        "screen and (min-width: 60em)",
        1280.0,
        900.0
    ));
    assert!(!matches_media_query_size(
        "screen and (min-width: 60em)",
        800.0,
        900.0
    ));
}

#[test]
fn test_media_and_container_queries() {
    // 1. prefers-color-scheme
    set_prefers_color_scheme(ColorSchemePreference::Dark);
    assert!(matches_media_query_size(
        "(prefers-color-scheme: dark)",
        1000.0,
        800.0
    ));
    assert!(!matches_media_query_size(
        "(prefers-color-scheme: light)",
        1000.0,
        800.0
    ));

    set_prefers_color_scheme(ColorSchemePreference::Light);
    assert!(!matches_media_query_size(
        "(prefers-color-scheme: dark)",
        1000.0,
        800.0
    ));
    assert!(matches_media_query_size(
        "(prefers-color-scheme: light)",
        1000.0,
        800.0
    ));

    // 2. prefers-reduced-motion
    set_prefers_reduced_motion(ReducedMotionPreference::Reduce);
    assert!(matches_media_query_size(
        "(prefers-reduced-motion: reduce)",
        1000.0,
        800.0
    ));
    assert!(!matches_media_query_size(
        "(prefers-reduced-motion: no-preference)",
        1000.0,
        800.0
    ));
    set_prefers_reduced_motion(ReducedMotionPreference::NoPreference);

    // 3. prefers-contrast
    set_prefers_contrast(ContrastPreference::More);
    assert!(matches_media_query_size(
        "(prefers-contrast: more)",
        1000.0,
        800.0
    ));
    assert!(!matches_media_query_size(
        "(prefers-contrast: less)",
        1000.0,
        800.0
    ));
    set_prefers_contrast(ContrastPreference::NoPreference);

    // 4. hover and pointer
    set_hover(HoverType::Hover);
    set_pointer(PointerType::Fine);
    assert!(matches_media_query_size("(hover: hover)", 1000.0, 800.0));
    assert!(matches_media_query_size("(hover)", 1000.0, 800.0));
    assert!(matches_media_query_size("(pointer: fine)", 1000.0, 800.0));
    assert!(matches_media_query_size("(pointer)", 1000.0, 800.0));

    // 5. resolution / device-pixel-ratio
    set_device_pixel_ratio(2.0);
    assert!(matches_media_query_size(
        "(min-resolution: 2dppx)",
        1000.0,
        800.0
    ));
    assert!(matches_media_query_size(
        "(min-resolution: 192dpi)",
        1000.0,
        800.0
    ));
    assert!(matches_media_query_size(
        "(-webkit-min-device-pixel-ratio: 2)",
        1000.0,
        800.0
    ));
    assert!(!matches_media_query_size(
        "(min-resolution: 3dppx)",
        1000.0,
        800.0
    ));
    set_device_pixel_ratio(1.0);

    // 6. Media Queries Level 4 range comparison syntax
    assert!(matches_media_query_size("(width >= 600px)", 800.0, 600.0));
    assert!(!matches_media_query_size("(width >= 900px)", 800.0, 600.0));
    assert!(matches_media_query_size(
        "(600px <= width <= 1000px)",
        800.0,
        600.0
    ));
    assert!(!matches_media_query_size(
        "(600px <= width <= 750px)",
        800.0,
        600.0
    ));
    assert!(matches_media_query_size(
        "(aspect-ratio >= 4/3)",
        800.0,
        600.0
    ));

    // 7. Logical 'or' and 'not'
    assert!(matches_media_query_size(
        "(max-width: 500px) or (min-width: 700px)",
        800.0,
        600.0
    ));
    assert!(!matches_media_query_size(
        "(max-width: 500px) or (min-width: 900px)",
        800.0,
        600.0
    ));
    assert!(matches_media_query_size(
        "not (max-width: 500px)",
        800.0,
        600.0
    ));

    // 8. Container queries matching and cascade resolution
    assert!(matches_container_query("(min-width: 400px)", 500.0, 300.0));
    assert!(!matches_container_query("(min-width: 600px)", 500.0, 300.0));
    assert!(matches_container_query(
        "(inline-size >= 400px)",
        500.0,
        300.0
    ));
    assert!(matches_container_query(
        "(300px <= width <= 600px)",
        500.0,
        300.0
    ));

    // 9. Cascade resolution with @container rule
    let html = r#"
        <div id="container" style="container-type: inline-size; width: 500px;">
            <div id="target" class="card">Hello</div>
        </div>
    "#;
    let doc = parse_html(html);
    let root = doc.root();
    let target_id = doc.find_element_by_id(root, "target").unwrap();

    let css = r#"
        .card { color: red; }
        @container (min-width: 400px) {
            .card { color: green; font-size: 20px; }
        }
        @container (min-width: 800px) {
            .card { color: blue; }
        }
    "#;
    let sheet = parse_stylesheet(css);
    let cascaded = resolve_cascade(target_id, &doc, &[&sheet]);

    // Container is 500px, so min-width: 400px matches (color: green), min-width: 800px does NOT match
    assert_eq!(
        cascaded.get("color"),
        Some(&Value::Color(Color::rgb(0, 128, 0)))
    );
    assert_eq!(
        cascaded.get("font-size"),
        Some(&Value::Length(Length::Px(20.0)))
    );
}

#[test]
fn test_media_query_rem_range() {
    assert!(matches_media_query("(width>=63.25rem)", 1280.0));
    assert!(!matches_media_query("(width>=63.25rem)", 800.0));
}
