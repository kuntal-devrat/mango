use super::*;

#[test]
fn test_base64_decode() {
    // "Hello" in base64 is "SGVsbG8="
    let decoded = base64_decode("SGVsbG8=").unwrap();
    assert_eq!(&decoded, b"Hello");
}

#[test]
fn test_base64_decode_no_padding() {
    let decoded = base64_decode("SGVsbG8").unwrap();
    assert_eq!(&decoded, b"Hello");
}

#[test]
fn test_create_placeholder() {
    let img = create_placeholder(64, 48);
    assert_eq!(img.width, 64);
    assert_eq!(img.height, 48);
    assert_eq!(img.pixels.len(), (64 * 48) as usize);

    // Check border pixels
    assert_eq!(img.pixels[0], 0xFFCCCCCC); // top-left
    assert_eq!(img.pixels[63], 0xFFCCCCCC); // top-right
}

#[test]
fn test_decode_data_uri_invalid() {
    assert!(decode_data_uri("not-a-data-uri").is_none());
    assert!(decode_data_uri("data:text/plain;base64,SGVsbG8=").is_none()); // text, not image
}

#[test]
fn test_create_placeholder_minimum_size() {
    let img = create_placeholder(2, 2);
    // Should be clamped to 16x16 minimum
    assert!(img.width >= 16);
    assert!(img.height >= 16);
}

#[test]
fn test_decode_tiny_png_data_uri() {
    // A valid 1x1 red PNG as base64 data URI
    let png_data = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
    let result = decode_data_uri(png_data);
    assert!(result.is_some(), "Should decode a valid PNG data URI");
    let img = result.unwrap();
    assert_eq!(img.width, 1);
    assert_eq!(img.height, 1);
    assert_eq!(img.pixels.len(), 1);
}

#[test]
fn test_resize_image() {
    let original = DecodedImage {
        width: 2,
        height: 2,
        pixels: vec![0xFFFF0000, 0xFF00FF00, 0xFF0000FF, 0xFFFFFF00],
    };
    let scaled = original.resize(4, 4);
    assert_eq!(scaled.width, 4);
    assert_eq!(scaled.height, 4);
    assert_eq!(scaled.pixels.len(), 16);
    assert_eq!(scaled.pixels[0], 0xFFFF0000);
}

#[test]
fn test_decode_svg_data_uri_with_charset() {
    let svg_data = "data:image/svg+xml;charset=utf-8,%3Csvg xmlns='http://www.w3.org/2000/svg' width='20' height='20'%3E%3Cpath d='M0 0h20v20H0z' fill='%23ff0000'/%3E%3C/svg%3E";
    let result = decode_data_uri(svg_data);
    assert!(
        result.is_some(),
        "Should decode SVG data URI with charset=utf-8"
    );
    let img = result.unwrap();
    assert_eq!(img.width, 20);
    assert_eq!(img.height, 20);
}

#[test]
fn test_decode_svg_data_uri_base64_with_charset() {
    let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><rect width='16' height='16' fill='blue'/></svg>";
    let b64 = base64_encode(svg.as_bytes());
    let uri = format!("data:image/svg+xml;charset=utf-8;base64,{b64}");
    let result = decode_data_uri(&uri);
    assert!(
        result.is_some(),
        "Should decode SVG data URI with charset and base64"
    );
}

#[test]
fn test_svg_without_intrinsic_dimensions_uses_html_default_object_size() {
    let uri = "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' />";
    let image = decode_data_uri(uri).expect("empty SVG image should still decode");
    assert_eq!((image.width, image.height), (300, 150));
}

#[test]
fn test_transparent_svg_stays_transparent_when_resized() {
    let uri = "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' />";
    let image = decode_data_uri(uri).expect("empty SVG image should decode");
    let resized = image.resize(15, 15);
    assert!(resized.pixels.iter().all(|pixel| (pixel >> 24) == 0));
}

#[test]
fn test_async_image_decode_and_placeholder() {
    let png_bytes = base64_decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==").unwrap();
    let key = "https://example.com/async_test_image.png";

    // Initial call returns placeholder immediately without blocking
    let initial = get_or_decode_async(key, &png_bytes, 32, 32);
    assert_eq!(initial.width, 32);
    assert_eq!(initial.height, 32);

    // Wait a short moment for the background worker to finish decoding
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Now image cache should contain the fully decoded 1x1 image
    let cached = get_cached_image(key).expect("Image should be in cache after async decode");
    assert_eq!(cached.width, 1);
    assert_eq!(cached.height, 1);
}

#[test]
fn test_progressive_jpeg_detection_and_scans() {
    // Construct a synthetic JPEG with SOF2 (Progressive DCT) marker: 0xFF 0xC2
    let progressive_header = [
        0xFF, 0xD8, // SOI
        0xFF, 0xE0, 0x00, 0x10, // APP0 length 16
        0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF,
        0xC2, 0x00, 0x0B, // SOF2 (Progressive DCT), length 11
        0x08, 0x00, 0x10, 0x00, 0x10, 0x03, 0x01, 0x11, 0x00, 0xFF, 0xDA, // SOS
        0x00, 0x00, 0xFF, 0xD9, // EOI
    ];
    assert!(is_progressive_jpeg(&progressive_header));

    // Baseline JPEG with SOF0 (0xFF 0xC0)
    let baseline_header = [
        0xFF, 0xD8, // SOI
        0xFF, 0xC0, 0x00, 0x0B, // SOF0 (Baseline DCT)
        0x08, 0x00, 0x10, 0x00, 0x10, 0x03, 0x01, 0x11, 0x00, 0xFF, 0xD9, // EOI
    ];
    assert!(!is_progressive_jpeg(&baseline_header));

    // Non-JPEG data
    assert!(!is_progressive_jpeg(b"not a jpeg"));
}

#[test]
fn test_animated_image_playback() {
    let frame1 = DecodedImage {
        width: 10,
        height: 10,
        pixels: vec![0xFFFF0000; 100], // Red
    };
    let frame2 = DecodedImage {
        width: 10,
        height: 10,
        pixels: vec![0xFF00FF00; 100], // Green
    };
    let frame3 = DecodedImage {
        width: 10,
        height: 10,
        pixels: vec![0xFF0000FF; 100], // Blue
    };

    let anim = AnimatedImage {
        width: 10,
        height: 10,
        frames: vec![
            AnimationFrame {
                image: frame1,
                delay_ms: 100,
            },
            AnimationFrame {
                image: frame2,
                delay_ms: 150,
            },
            AnimationFrame {
                image: frame3,
                delay_ms: 200,
            },
        ],
        loop_count: 0,
        total_duration_ms: 450,
    };

    // At 0ms -> Frame 1 (Red)
    assert_eq!(anim.frame_at_time(0).pixels[0], 0xFFFF0000);
    // At 50ms -> Frame 1 (Red)
    assert_eq!(anim.frame_at_time(50).pixels[0], 0xFFFF0000);
    // At 120ms -> Frame 2 (Green)
    assert_eq!(anim.frame_at_time(120).pixels[0], 0xFF00FF00);
    // At 240ms -> Frame 2 (Green)
    assert_eq!(anim.frame_at_time(240).pixels[0], 0xFF00FF00);
    // At 300ms -> Frame 3 (Blue)
    assert_eq!(anim.frame_at_time(300).pixels[0], 0xFF0000FF);
    // At 460ms -> Loops back to Frame 1 (Red, since 460 % 450 = 10ms)
    assert_eq!(anim.frame_at_time(460).pixels[0], 0xFFFF0000);

    // Test caching
    let key = "https://example.com/test.gif";
    cache_animated_image(key, anim);
    let cached_anim = get_cached_animated_image(key);
    assert!(cached_anim.is_some());
    let frame = get_animated_frame(key, 120);
    assert!(frame.is_some());
    assert_eq!(frame.unwrap().pixels[0], 0xFF00FF00);
}

#[test]
fn test_bounded_image_store_lru_eviction() {
    let make_img = |pixel: u32| DecodedImage {
        width: 10,
        height: 10,
        pixels: vec![pixel; 100], // 100 * 4 = 400 bytes
    };

    // Budget for exactly two 400-byte images
    let mut store = BoundedImageStore::new(800);

    store.insert("img1".to_string(), make_img(1));
    store.insert("img2".to_string(), make_img(2));
    assert_eq!(store.total_bytes, 800);

    // Access img1 so it becomes more recently used than img2
    assert!(store.get(&"img1".to_string()).is_some());

    // Insert img3 - must evict the least recently used item (img2)
    store.insert("img3".to_string(), make_img(3));
    assert_eq!(store.total_bytes, 800);

    assert!(
        store.get(&"img2".to_string()).is_none(),
        "img2 should have been evicted by LRU"
    );
    assert!(
        store.get(&"img1".to_string()).is_some(),
        "img1 should remain in cache"
    );
    assert!(
        store.get(&"img3".to_string()).is_some(),
        "img3 should be in cache"
    );
}
