use mango_core::Color;

use super::loader::FONT_SANS;
use super::*;

#[test]
fn test_font_manager_initializes() {
    let fm = font_manager();
    let (w, h) = fm.measure_text("Hello", 16.0, FontWeight::Regular, FontFamily::SansSerif);
    assert!(w > 0.0, "text width should be positive");
    assert!(h > 0.0, "text height should be positive");
}

#[test]
fn test_proportional_measurement() {
    let fm = font_manager();
    let (w_wide, _) = fm.measure_text("WWWWW", 16.0, FontWeight::Regular, FontFamily::SansSerif);
    let (w_narrow, _) = fm.measure_text("iiiii", 16.0, FontWeight::Regular, FontFamily::SansSerif);
    assert!(
        w_wide > w_narrow,
        "W ({w_wide}) should be wider than i ({w_narrow})"
    );
}

#[test]
fn test_bold_measurement() {
    let fm = font_manager();
    let (w_regular, _) = fm.measure_text("Hello", 16.0, FontWeight::Regular, FontFamily::SansSerif);
    let (w_bold, _) = fm.measure_text("Hello", 16.0, FontWeight::Bold, FontFamily::SansSerif);
    println!("w_regular={}, w_bold={}", w_regular, w_bold);
    assert!(w_regular > 0.0);
    assert!(w_bold > 0.0);
}

#[test]
fn test_serif_measurement() {
    let fm = font_manager();
    let (w_serif, h_serif) =
        fm.measure_text("Moby-Dick", 16.0, FontWeight::Regular, FontFamily::Serif);
    let (w_serif_bold, _) = fm.measure_text("Moby-Dick", 16.0, FontWeight::Bold, FontFamily::Serif);
    assert!(w_serif > 0.0, "serif width should be positive");
    assert!(h_serif > 0.0, "serif height should be positive");
    assert!(w_serif_bold > 0.0, "serif bold width should be positive");
}

#[test]
fn test_mono_measurement() {
    let fm = font_manager();
    let w_a = fm
        .select_font(FontFamily::Monospace, FontWeight::Regular)
        .metrics('a', 16.0)
        .advance_width;
    let w_m = fm
        .select_font(FontFamily::Monospace, FontWeight::Regular)
        .metrics('M', 16.0)
        .advance_width;
    assert!(
        (w_a - w_m).abs() < 0.01,
        "monospace chars should have equal width: a={w_a}, M={w_m}"
    );
}

#[test]
fn test_rasterize_text_no_panic() {
    let fm = font_manager();
    let mut buffer = vec![0xFFFFFFu32; 200 * 50];
    fm.rasterize_text(
        &mut buffer,
        200,
        50,
        5,
        5,
        "Hello World!",
        Color::BLACK,
        16.0,
        FontWeight::Regular,
        FontFamily::SansSerif,
        FontStyle::Normal,
        0.0,
    );
    let modified = buffer.iter().filter(|&&px| px != 0xFFFFFF).count();
    assert!(
        modified > 0,
        "rasterize_text should have painted some pixels"
    );
}

#[test]
fn test_resolve_family() {
    assert_eq!(
        FontManager::resolve_family("monospace"),
        FontFamily::Monospace
    );
    assert_eq!(
        FontManager::resolve_family("Courier New"),
        FontFamily::Monospace
    );
    assert_eq!(FontManager::resolve_family("Arial"), FontFamily::SansSerif);
    assert_eq!(
        FontManager::resolve_family("sans-serif"),
        FontFamily::SansSerif
    );
    assert_eq!(FontManager::resolve_family("serif"), FontFamily::Serif);
    assert_eq!(
        FontManager::resolve_family("Times New Roman"),
        FontFamily::Serif
    );
    let georgia = FontManager::resolve_family("Georgia");
    assert!(
        georgia == FontFamily::Serif || matches!(georgia, FontFamily::Custom(_)),
        "Georgia should resolve to Custom(id) if installed on system, or fallback to Serif"
    );
}

#[test]
fn test_web_font_registration_and_resolution() {
    let fm = font_manager();
    assert!(!fm.has_web_font("CustomWikipediaSans"));

    let font_id = fm
        .register_web_font("CustomWikipediaSans", FontWeight::Regular, FONT_SANS)
        .expect("should register TTF bytes as web font");

    assert!(fm.has_web_font("CustomWikipediaSans"));
    assert!(fm.has_web_font("customwikipediasans"));

    let resolved = fm.resolve_family_for("'CustomWikipediaSans', sans-serif");
    assert_eq!(resolved, FontFamily::Custom(font_id));

    let (w, h) = fm.measure_text("Wikipedia Parity", 18.0, FontWeight::Regular, resolved);
    assert!(w > 0.0);
    assert!(h > 0.0);
}

#[test]
fn test_web_font_registration_via_data_uri() {
    let fm = font_manager();
    let result = fm.register_web_font(
        "BadFont",
        FontWeight::Regular,
        b"data:font/woff2;base64,AAAA",
    );
    assert!(result.is_err());
}

#[test]
fn test_glyph_caching() {
    let fm = font_manager();
    let font = fm.select_font(FontFamily::SansSerif, FontWeight::Regular);
    let g1 = fm.get_or_rasterize_glyph(FontFamily::SansSerif, FontWeight::Regular, font, 'M', 16.0);
    let g2 = fm.get_or_rasterize_glyph(FontFamily::SansSerif, FontWeight::Regular, font, 'M', 16.0);
    assert_eq!(g1.metrics.width, g2.metrics.width);
    assert_eq!(g1.metrics.height, g2.metrics.height);
    assert_eq!(g1.bitmap.len(), g2.bitmap.len());
    assert!(std::sync::Arc::ptr_eq(&g1.bitmap, &g2.bitmap));
}

#[test]
fn test_indic_font_fallback() {
    let fm = font_manager();
    let hindi_text = "हिन्दी";
    let (w, h) = fm.measure_text(hindi_text, 14.0, FontWeight::Regular, FontFamily::SansSerif);
    assert!(w > 0.0);
    assert!(h > 0.0);

    let mut buf = vec![0u32; 200 * 50];
    fm.rasterize_text(
        &mut buf,
        200,
        50,
        0,
        0,
        hindi_text,
        Color::WHITE,
        14.0,
        FontWeight::Regular,
        FontFamily::SansSerif,
        FontStyle::Normal,
        0.0,
    );
    assert!(buf.iter().any(|&p| p != 0));
}

#[test]
fn test_multilingual_font_fallback() {
    let fm = font_manager();
    let samples = [
        ("Chinese", "芒果浏览器"),
        ("Japanese", "こんにちは世界"),
        ("Korean", "안녕하세요"),
        ("Arabic", "العربية"),
        ("Thai", "ภาษาไทย"),
        ("Hebrew", "שלום עולם"),
        ("Emoji", "🥭🚀"),
    ];

    for (lang, text) in samples {
        let (w, h) = fm.measure_text(text, 16.0, FontWeight::Regular, FontFamily::SansSerif);
        assert!(w > 0.0, "Language {lang} width must be > 0, got {w}");
        assert!(h > 0.0, "Language {lang} height must be > 0, got {h}");

        let mut buf = vec![0u32; 300 * 60];
        fm.rasterize_text(
            &mut buf,
            300,
            60,
            0,
            0,
            text,
            Color::WHITE,
            16.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
            FontStyle::Normal,
            0.0,
        );
        assert!(
            buf.iter().any(|&p| p != 0),
            "Language {lang} must rasterize non-zero pixels"
        );
    }
}

#[test]
fn test_subpixel_text_rendering() {
    let fm = font_manager();
    let mut buffer = vec![0xFFFFFFu32; 200 * 50];
    fm.rasterize_text_subpixel(
        &mut buffer,
        200,
        50,
        10,
        10,
        "ClearType Subpixel",
        Color::BLACK,
        16.0,
        FontWeight::Regular,
        FontFamily::SansSerif,
        FontStyle::Normal,
        0.0,
    );

    let modified = buffer.iter().filter(|&&px| px != 0xFFFFFF).count();
    assert!(modified > 0, "Subpixel rendering must modify buffer pixels");

    let has_colored_subpixel_edges = buffer.iter().any(|&px| {
        let r = (px >> 16) & 0xFF;
        let g = (px >> 8) & 0xFF;
        let b = px & 0xFF;
        r != g || g != b
    });
    assert!(
        has_colored_subpixel_edges,
        "Subpixel rendering must yield distinct R, G, B channels on edge pixels"
    );
}

#[test]
fn test_color_emoji_rendering() {
    assert!(is_emoji_char('🥭'));
    assert!(is_emoji_char('🚀'));
    assert!(is_emoji_char('😀'));
    assert!(is_emoji_char('🔥'));
    assert!(is_emoji_char('🎉'));
    assert!(is_emoji_char('⭐'));
    assert!(is_emoji_char('\u{2764}'));
    assert!(!is_emoji_char('A'));
    assert!(!is_emoji_char('7'));

    let mango_emoji = rasterize_color_emoji('🥭', 24);
    assert_eq!(mango_emoji.width, 24);
    assert_eq!(mango_emoji.height, 24);
    assert!(!mango_emoji.pixels.is_empty());

    let has_warm_orange = mango_emoji.pixels.iter().any(|&p| {
        let r = (p >> 16) & 0xFF;
        let g = (p >> 8) & 0xFF;
        let b = p & 0xFF;
        r > 200 && g > 80 && g < 220 && b < 80
    });
    let has_green_leaf = mango_emoji.pixels.iter().any(|&p| {
        let r = (p >> 16) & 0xFF;
        let g = (p >> 8) & 0xFF;
        let b = p & 0xFF;
        g > r && g > b && g > 80
    });
    assert!(
        has_warm_orange,
        "Mango emoji must contain golden/orange body pixels"
    );
    assert!(has_green_leaf, "Mango emoji must contain green leaf pixels");

    let fm = font_manager();
    let mut buf = vec![0xFFFFFFu32; 100 * 40];
    fm.rasterize_text(
        &mut buf,
        100,
        40,
        5,
        5,
        "🥭 Rocket 🚀",
        Color::BLACK,
        20.0,
        FontWeight::Regular,
        FontFamily::SansSerif,
        FontStyle::Normal,
        0.0,
    );
    let non_white = buf.iter().filter(|&&p| p != 0xFFFFFF).count();
    assert!(non_white > 0, "Emoji text rasterization must draw pixels");
}

#[test]
fn test_variable_font_axis_interpolation() {
    let mut ttf = Vec::new();
    ttf.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
    ttf.extend_from_slice(&1u16.to_be_bytes());
    ttf.extend_from_slice(&[0x00, 0x10, 0x00, 0x00, 0x00, 0x00]);

    let fvar_offset = 12 + 16;
    ttf.extend_from_slice(b"fvar");
    ttf.extend_from_slice(&0u32.to_be_bytes());
    ttf.extend_from_slice(&(fvar_offset as u32).to_be_bytes());
    let fvar_len = 16 + 20;
    ttf.extend_from_slice(&(fvar_len as u32).to_be_bytes());

    ttf.extend_from_slice(&1u16.to_be_bytes());
    ttf.extend_from_slice(&0u16.to_be_bytes());
    ttf.extend_from_slice(&16u16.to_be_bytes());
    ttf.extend_from_slice(&2u16.to_be_bytes());
    ttf.extend_from_slice(&1u16.to_be_bytes());
    ttf.extend_from_slice(&20u16.to_be_bytes());
    ttf.extend_from_slice(&0u16.to_be_bytes());
    ttf.extend_from_slice(&0u16.to_be_bytes());

    ttf.extend_from_slice(b"wght");
    ttf.extend_from_slice(&(100i32 * 65536).to_be_bytes());
    ttf.extend_from_slice(&(400i32 * 65536).to_be_bytes());
    ttf.extend_from_slice(&(900i32 * 65536).to_be_bytes());
    ttf.extend_from_slice(&0u16.to_be_bytes());
    ttf.extend_from_slice(&256u16.to_be_bytes());

    let meta = parse_fvar_table(&ttf).expect("fvar table should parse successfully");
    assert_eq!(meta.axes.len(), 1);
    let wght = &meta.axes[0];
    assert_eq!(&wght.tag, b"wght");
    assert_eq!(wght.tag_str, "wght");
    assert!((wght.min_value - 100.0).abs() < 0.01);
    assert!((wght.default_value - 400.0).abs() < 0.01);
    assert!((wght.max_value - 900.0).abs() < 0.01);

    assert_eq!(interpolate_axis(wght, 400.0), 0.0);
    assert!((interpolate_axis(wght, 100.0) - (-1.0)).abs() < 0.001);
    assert!((interpolate_axis(wght, 900.0) - 1.0).abs() < 0.001);
    assert!((interpolate_axis(wght, 650.0) - 0.5).abs() < 0.001);
}

#[test]
fn test_render_ddg_snippet_glyphs() {
    let fm = font_manager();
    let family = fm.resolve_family_for("sans-serif");
    let text = "while guaranteeing memory-safety and thread-safety, empowering developers to debug at compile-time. In addition to that Rust has great documentation and a user-friendly";
    let mut buf = vec![0xFFFFFFFFu32; 1000 * 100];
    fm.rasterize_text_clipped(
        &mut buf,
        1000,
        100,
        10,
        20,
        text,
        Color::BLACK,
        14.0,
        FontWeight::Regular,
        family,
        FontStyle::Normal,
        0.0,
        mango_core::Rect::new(0.0, 0.0, 1000.0, 100.0),
    );
    let mut img = image::RgbImage::new(1000, 100);
    for y in 0..100 {
        for x in 0..1000 {
            let px = buf[y * 1000 + x];
            let r = ((px >> 16) & 0xFF) as u8;
            let g = ((px >> 8) & 0xFF) as u8;
            let b = (px & 0xFF) as u8;
            img.put_pixel(x as u32, y as u32, image::Rgb([r, g, b]));
        }
    }
    let path = std::env::temp_dir().join("mango_text_test.png");
    let _ = img.save(path);
}

#[test]
fn test_web_font_multiple_slices() {
    if let Ok(bytes) = std::fs::read("/tmp/fira.woff2") {
        let fm = font_manager();
        let id = fm
            .register_web_font("TestSliceFamily", FontWeight::Regular, &bytes)
            .unwrap();
        let (w, _) = fm.measure_text_with_spacing(
            "Install",
            18.0,
            FontWeight::Regular,
            FontFamily::Custom(id),
            0.0,
        );
        assert!((w - 51.1).abs() < 1.0);
    }
}

#[test]
fn test_prune_web_fonts() {
    let fm = FontManager::new();
    let family_name = "PrunableWebFont";
    let id = fm
        .register_web_font(family_name, FontWeight::Regular, FONT_SANS)
        .expect("register web font should succeed");

    assert!(fm.has_web_font(family_name));
    let resolved = fm.resolve_family_for(family_name);
    assert_eq!(resolved, FontFamily::Custom(id));

    fm.prune_web_fonts();
    assert!(!fm.has_web_font(family_name));

    let sans = fm.resolve_family_for("sans-serif");
    assert_eq!(sans, FontFamily::SansSerif);
    let (w, h) = fm.measure_text("Hello", 16.0, FontWeight::Regular, sans);
    assert!(w > 0.0 && h > 0.0);
}
