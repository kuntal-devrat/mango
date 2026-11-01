use mango_core::Color;

use super::cache::{
    clear_global_svg_symbols, get_svg_intrinsic_dimensions, render_svg, svg_doc_cache,
    svg_render_cache,
};
use super::path::parse_svg_path;

#[test]
fn test_render_simple_svg_rect() {
    let svg = r##"<svg viewBox="0 0 100 100"><rect x="10" y="10" width="80" height="80" fill="#FF0000"/></svg>"##;
    let img = render_svg(svg, 100, 100, Color::BLACK).expect("SVG should render");
    assert_eq!(img.width, 100);
    assert_eq!(img.height, 100);

    // Center pixel should be red (alpha = 255, red = 255)
    let center_idx = 50 * 100 + 50;
    let center_pixel = img.pixels[center_idx];
    let alpha = (center_pixel >> 24) & 0xFF;
    let red = (center_pixel >> 16) & 0xFF;
    assert_eq!(alpha, 255);
    assert_eq!(red, 255);
}

#[test]
fn test_render_svg_path_current_color() {
    let svg = r#"<svg viewBox="0 0 24 24"><path d="M0 0 H24 V24 H0 Z" fill="currentColor"/></svg>"#;
    let custom_color = Color::rgb(40, 120, 200);
    let img = render_svg(svg, 24, 24, custom_color).expect("SVG should render");
    let p = img.pixels[12 * 24 + 12];
    let r = (p >> 16) & 0xFF;
    let g = (p >> 8) & 0xFF;
    let b = p & 0xFF;
    assert_eq!(r, 40);
    assert_eq!(g, 120);
    assert_eq!(b, 200);
}

#[test]
fn test_parse_svg_path_arcs() {
    let d = "M10 80 Q 52.5 10, 95 80 T 180 80";
    let path = parse_svg_path(d);
    assert!(path.is_some());
}

#[test]
fn test_svg_defs_symbol_and_use_intra_document() {
    let svg = r##"
    <svg viewBox="0 0 100 100">
        <defs>
            <symbol id="box-sym" viewBox="0 0 50 50">
                <rect x="0" y="0" width="50" height="50" fill="#00FF00"/>
            </symbol>
        </defs>
        <use xlink:href="#box-sym" x="10" y="10" width="80" height="80"/>
    </svg>
    "##;
    let img = render_svg(svg, 100, 100, Color::BLACK).expect("SVG with use should render");
    let center_idx = 50 * 100 + 50;
    let center_pixel = img.pixels[center_idx];
    let alpha = (center_pixel >> 24) & 0xFF;
    let green = (center_pixel >> 8) & 0xFF;
    assert_eq!(alpha, 255);
    assert_eq!(green, 255);
}

#[test]
fn test_svg_symbol_cross_document_sharing() {
    clear_global_svg_symbols();

    // 1. First SVG defines a symbol in a hidden sprite (like Wikipedia)
    let sprite_svg = r##"
    <svg style="display:none">
        <defs>
            <symbol id="global-star" viewBox="0 0 20 20">
                <rect x="0" y="0" width="20" height="20" fill="#0000FF"/>
            </symbol>
        </defs>
    </svg>
    "##;
    let _ = render_svg(sprite_svg, 20, 20, Color::BLACK);

    // 2. Second SVG references #global-star without defining it
    let consumer_svg = r##"
    <svg viewBox="0 0 40 40">
        <use href="#global-star" x="0" y="0" width="40" height="40"/>
    </svg>
    "##;
    let img =
        render_svg(consumer_svg, 40, 40, Color::BLACK).expect("Cross-SVG symbol should resolve");
    let center_idx = 20 * 40 + 20;
    let center_pixel = img.pixels[center_idx];
    let blue = center_pixel & 0xFF;
    assert_eq!(blue, 255);
}

#[test]
fn test_svg_use_current_color_propagation() {
    let svg = r##"
    <svg viewBox="0 0 20 20">
        <defs>
            <symbol id="icon-fill">
                <rect x="0" y="0" width="20" height="20"/>
            </symbol>
        </defs>
        <use href="#icon-fill" fill="currentColor"/>
    </svg>
    "##;
    let orange = Color::rgb(255, 165, 0);
    let img = render_svg(svg, 20, 20, orange).expect("SVG use currentColor should render");
    let p = img.pixels[10 * 20 + 10];
    let r = (p >> 16) & 0xFF;
    let g = (p >> 8) & 0xFF;
    let b = p & 0xFF;
    assert_eq!(r, 255);
    assert_eq!(g, 165);
    assert_eq!(b, 0);
}

#[test]
fn test_svg_root_current_color_inheritance() {
    let svg = r##"
    <svg fill="currentColor" viewBox="0 0 16 16">
        <path d="M0 0h16v16H0z"/>
    </svg>
    "##;
    let blue = Color::rgb(84, 174, 255);
    let img = render_svg(svg, 16, 16, blue).expect("SVG root currentColor should render");
    let p = img.pixels[8 * 16 + 8];
    let r = (p >> 16) & 0xFF;
    let g = (p >> 8) & 0xFF;
    let b = p & 0xFF;
    assert_eq!(r, 84);
    assert_eq!(g, 174);
    assert_eq!(b, 255);
}

#[test]
fn test_svg_use_intrinsic_dimensions_fallback() {
    let svg = r##"
    <svg>
        <defs>
            <symbol id="dim-icon" viewBox="0 0 64 32">
                <rect x="0" y="0" width="64" height="32"/>
            </symbol>
        </defs>
        <use href="#dim-icon"/>
    </svg>
    "##;
    let (w, h) = get_svg_intrinsic_dimensions(svg);
    assert_eq!(w, Some(64.0));
    assert_eq!(h, Some(32.0));
}

#[test]
fn test_svg_rendering_and_dom_caching() {
    let svg = r##"<svg viewBox="0 0 10 10"><rect x="0" y="0" width="10" height="10" fill="#123456"/></svg>"##;
    let img1 = render_svg(svg, 10, 10, Color::BLACK).expect("should render SVG");
    let img2 = render_svg(svg, 10, 10, Color::BLACK).expect("should return cached rendered SVG");

    assert_eq!(img1.width, img2.width);
    assert_eq!(img1.height, img2.height);
    assert_eq!(img1.pixels, img2.pixels);

    // Verify that the parsed DOM cache contains this SVG XML
    let cache = svg_doc_cache().read().unwrap();
    assert!(cache.contains_key(svg));

    // Verify that the render cache contains this key
    let rcache = svg_render_cache().read().unwrap();
    assert!(rcache.contains_key(&(svg.to_string(), 10, 10, Color::BLACK.to_argb_u32())));
}

#[test]
fn test_svg_text_and_tspan_rendering() {
    let svg = r##"
    <svg viewBox="0 0 120 50">
        <text x="10" y="35" font-size="18" fill="#FF0000">Hi <tspan fill="#0000FF">Mango</tspan></text>
    </svg>
    "##;
    let img = render_svg(svg, 120, 50, Color::BLACK).expect("SVG text should render");
    assert_eq!(img.width, 120);
    assert_eq!(img.height, 50);

    // Verify non-empty rasterization: some pixels must have red and blue glyph coverage
    let mut has_red = false;
    let mut has_blue = false;
    for &p in &img.pixels {
        let a = (p >> 24) & 0xFF;
        let r = (p >> 16) & 0xFF;
        let b = p & 0xFF;
        if a > 0 && r > 100 {
            has_red = true;
        }
        if a > 0 && b > 100 {
            has_blue = true;
        }
    }
    assert!(has_red, "Should render red main text glyphs");
    assert!(has_blue, "Should render blue tspan glyphs");
}

#[test]
fn test_svg_linear_gradient_fill() {
    let svg = r##"
    <svg viewBox="0 0 100 20">
        <defs>
            <linearGradient id="grad1" x1="0%" y1="0%" x2="100%" y2="0%">
                <stop offset="0%" stop-color="#FF0000" />
                <stop offset="100%" stop-color="#0000FF" />
            </linearGradient>
        </defs>
        <rect x="0" y="0" width="100" height="20" fill="url(#grad1)" />
    </svg>
    "##;
    let img = render_svg(svg, 100, 20, Color::BLACK).expect("SVG linear gradient should render");
    // Pixel near left edge (5, 10) should be predominantly red
    let left_pixel = img.pixels[10 * 100 + 5];
    let left_r = (left_pixel >> 16) & 0xFF;
    let left_b = left_pixel & 0xFF;
    assert!(left_r > 200, "Left side should be red: got r={left_r}");
    assert!(left_b < 50, "Left side should not be blue: got b={left_b}");

    // Pixel near right edge (95, 10) should be predominantly blue
    let right_pixel = img.pixels[10 * 100 + 95];
    let right_r = (right_pixel >> 16) & 0xFF;
    let right_b = right_pixel & 0xFF;
    assert!(right_b > 200, "Right side should be blue: got b={right_b}");
    assert!(
        right_r < 50,
        "Right side should not be red: got r={right_r}"
    );
}

#[test]
fn test_svg_radial_gradient_fill() {
    let svg = r##"
    <svg viewBox="0 0 60 60">
        <defs>
            <radialGradient id="rad1" cx="50%" cy="50%" r="50%">
                <stop offset="0%" stop-color="#00FF00" />
                <stop offset="100%" stop-color="#000000" />
            </radialGradient>
        </defs>
        <circle cx="30" cy="30" r="30" fill="url(#rad1)" />
    </svg>
    "##;
    let img = render_svg(svg, 60, 60, Color::BLACK).expect("SVG radial gradient should render");
    // Center pixel (30, 30) should be green
    let center_pixel = img.pixels[30 * 60 + 30];
    let g = (center_pixel >> 8) & 0xFF;
    assert!(g > 200, "Center should be green: got g={g}");
}

#[test]
fn test_svg_clip_path_rendering() {
    let svg = r##"
    <svg viewBox="0 0 100 100">
        <defs>
            <clipPath id="circle-clip">
                <circle cx="50" cy="50" r="20" />
            </clipPath>
        </defs>
        <rect x="0" y="0" width="100" height="100" fill="#FF0000" clip-path="url(#circle-clip)" />
    </svg>
    "##;
    let img = render_svg(svg, 100, 100, Color::BLACK).expect("SVG clipPath should render");

    // Center pixel (50, 50) is inside the circle: must be red
    let center_p = img.pixels[50 * 100 + 50];
    let center_a = (center_p >> 24) & 0xFF;
    let center_r = (center_p >> 16) & 0xFF;
    assert_eq!(center_a, 255);
    assert_eq!(center_r, 255);

    // Corner pixel (5, 5) is outside the circle clip: must remain transparent
    let corner_p = img.pixels[5 * 100 + 5];
    let corner_a = (corner_p >> 24) & 0xFF;
    assert_eq!(corner_a, 0, "Pixel outside clipPath should be transparent");
}

#[test]
fn test_svg_stroke_dasharray_and_offset() {
    let svg = r##"
    <svg viewBox="0 0 100 20">
        <line x1="0" y1="10" x2="100" y2="10" stroke="#00FF00" stroke-width="6" stroke-dasharray="20, 20" />
    </svg>
    "##;
    let img = render_svg(svg, 100, 20, Color::BLACK).expect("SVG stroke-dasharray should render");

    // Pixel in the first dash [0..20], e.g. at (10, 10): must be green
    let dash_pixel = img.pixels[10 * 100 + 10];
    let dash_a = (dash_pixel >> 24) & 0xFF;
    let dash_g = (dash_pixel >> 8) & 0xFF;
    assert!(dash_a > 200);
    assert!(dash_g > 200);

    // Pixel in the gap [20..40], e.g. at (30, 10): must be transparent
    let gap_pixel = img.pixels[10 * 100 + 30];
    let gap_a = (gap_pixel >> 24) & 0xFF;
    assert_eq!(
        gap_a, 0,
        "Pixel inside stroke dash gap should be transparent"
    );
}

#[test]
fn test_svg_preserve_aspect_ratio_modes() {
    // preserveAspectRatio="none" stretches non-square viewBox to fill destination
    let svg = r##"<svg viewBox="0 0 10 10" preserveAspectRatio="none"><rect x="0" y="0" width="10" height="10" fill="#0000FF"/></svg>"##;
    let img =
        render_svg(svg, 100, 40, Color::BLACK).expect("SVG preserveAspectRatio should render");
    assert_eq!(img.width, 100);
    assert_eq!(img.height, 40);

    // Every pixel in 100x40 should be filled with blue because it stretched with "none"
    let p_top_left = img.pixels[0];
    let p_bottom_right = img.pixels[39 * 100 + 99];
    assert_eq!(p_top_left & 0xFF, 255);
    assert_eq!(p_bottom_right & 0xFF, 255);
}
