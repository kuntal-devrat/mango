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
