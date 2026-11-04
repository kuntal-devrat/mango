//! Adapter for rendering SVGs using `resvg` and `usvg`.
//!
//! Provides full SVG 1.1 and SVG 2 vector rendering (complex paths, gradients,
//! clip paths, patterns, masks, markers, text, and SVG icons/graphs).

use std::sync::{Arc, OnceLock};

use mango_core::Color;
use tiny_skia::{Color as SkiaColor, Pixmap, Transform};
use usvg::{Options, Tree};

use crate::font::font_manager;
use crate::image_decode::DecodedImage;

static RESVG_FONTDB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();

fn get_resvg_fontdb() -> Arc<usvg::fontdb::Database> {
    RESVG_FONTDB
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            let fm = font_manager();
            db.load_font_data(fm.sans_regular_data.to_vec());
            db.load_font_data(fm.sans_bold_data.to_vec());
            db.load_font_data(fm.serif_regular_data.to_vec());
            db.load_font_data(fm.serif_bold_data.to_vec());
            db.load_font_data(fm.mono_data.to_vec());
            db.set_sans_serif_family("Liberation Sans");
            db.set_serif_family("Liberation Serif");
            db.set_monospace_family("Liberation Mono");
            Arc::new(db)
        })
        .clone()
}

/// Extracts intrinsic dimensions (width, height) using `usvg`.
pub fn get_svg_intrinsic_dimensions_resvg(svg_xml: &str) -> Option<(f32, f32)> {
    if svg_xml.trim().is_empty() {
        return None;
    }
    let opt = Options::default();
    let tree = Tree::from_str(svg_xml, &opt).ok()?;
    let size = tree.size();
    if size.width() > 0.0 && size.height() > 0.0 {
        Some((size.width(), size.height()))
    } else {
        None
    }
}

/// Renders an SVG into a [`DecodedImage`] using `resvg`.
pub fn render_svg_resvg(
    svg_xml: &str,
    target_w: u32,
    target_h: u32,
    current_color: Color,
) -> Option<DecodedImage> {
    if target_w == 0 || target_h == 0 || svg_xml.trim().is_empty() {
        return None;
    }

    let mut opt = Options::default();
    opt.dpi = 96.0;
    opt.fontdb = get_resvg_fontdb();
    opt.font_family = "Liberation Sans".to_string();

    // Replace currentColor keyword if present with resolved RGB hex
    let processed_svg;
    let input_xml = if svg_xml.contains("currentColor") {
        let hex = format!(
            "#{:02x}{:02x}{:02x}",
            current_color.r, current_color.g, current_color.b
        );
        processed_svg = svg_xml.replace("currentColor", &hex);
        &processed_svg
    } else {
        svg_xml
    };

    let tree = Tree::from_str(input_xml, &opt).ok()?;

    let mut pixmap = Pixmap::new(target_w, target_h)?;
    pixmap.fill(SkiaColor::TRANSPARENT);

    let svg_size = tree.size();
    if svg_size.width() <= 0.0 || svg_size.height() <= 0.0 {
        return None;
    }

    let scale_x = target_w as f32 / svg_size.width();
    let scale_y = target_h as f32 / svg_size.height();
    let transform = Transform::from_scale(scale_x, scale_y);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let mut pixels = Vec::with_capacity((target_w * target_h) as usize);
    for pixel in pixmap.pixels() {
        let a = pixel.alpha();
        let r = pixel.red();
        let g = pixel.green();
        let b = pixel.blue();
        pixels.push(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32));
    }

    Some(DecodedImage {
        width: target_w,
        height: target_h,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resvg_render_simple_rect() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect width="100" height="100" fill="#ff0000" />
        </svg>"##;

        let img = render_svg_resvg(svg, 100, 100, Color::BLACK).expect("resvg should render rect");
        assert_eq!(img.width, 100);
        assert_eq!(img.height, 100);
        // Center pixel should be red (0xffff0000)
        let center_pixel = img.pixels[50 * 100 + 50];
        assert_eq!(center_pixel, 0xffff0000);
    }

    #[test]
    fn test_resvg_current_color_substitution() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
            <circle cx="25" cy="25" r="20" fill="currentColor" />
        </svg>"#;

        let blue = Color::rgb(0, 0, 255);
        let img = render_svg_resvg(svg, 50, 50, blue).expect("resvg should render circle");
        let center = img.pixels[25 * 50 + 25];
        assert_eq!(center, 0xff0000ff);
    }

    #[test]
    fn test_resvg_symbol_in_defs() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40">
            <defs>
                <symbol id="global-star" viewBox="0 0 20 20">
                    <rect x="0" y="0" width="20" height="20" fill="#0000FF"/>
                </symbol>
            </defs>
            <use href="#global-star" x="0" y="0" width="40" height="40"/>
        </svg>"##;
        let img = render_svg_resvg(svg, 40, 40, Color::BLACK)
            .expect("resvg should render symbol in defs");
        let center = img.pixels[20 * 40 + 20];
        let blue = center & 0xFF;
        assert_eq!(blue, 255);
    }

    #[test]
    fn test_resvg_intrinsic_dimensions() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100"></svg>"#;
        let dims = get_svg_intrinsic_dimensions_resvg(svg);
        assert_eq!(dims, Some((200.0, 100.0)));

        let svg_sym = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <defs>
                <symbol id="dim-icon" viewBox="0 0 64 32">
                    <rect x="0" y="0" width="64" height="32"/>
                </symbol>
            </defs>
            <use href="#dim-icon"/>
        </svg>"##;
        let dims_sym = get_svg_intrinsic_dimensions_resvg(svg_sym);
        println!("Dims sym: {:?}", dims_sym);
    }

    #[test]
    fn test_resvg_text_rendering() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 50">
            <text x="10" y="35" font-size="18" fill="#ff0000">Hi <tspan fill="#0000ff">Mango</tspan></text>
        </svg>"##;
        let img = render_svg_resvg(svg, 120, 50, Color::BLACK).expect("resvg should render text");
        assert_eq!(img.width, 120);
        assert_eq!(img.height, 50);

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
        assert!(has_red, "Should render red main text glyphs in resvg");
        assert!(has_blue, "Should render blue tspan glyphs in resvg");
    }
}
