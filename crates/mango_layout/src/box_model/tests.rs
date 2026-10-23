use super::*;
use mango_core::geometry::{EdgeSizes, Rect};
use mango_css::values::ObjectFit;

#[test]
fn test_box_type_predicates() {
    assert!(BoxType::BlockNode.is_block());
    assert!(!BoxType::BlockNode.is_inline());
    assert!(BoxType::BlockNode.is_container());

    assert!(BoxType::AnonymousBlock.is_block());
    assert!(BoxType::AnonymousBlock.is_anonymous());

    assert!(BoxType::InlineNode.is_inline());
    assert!(!BoxType::InlineNode.is_block());
    assert!(!BoxType::InlineNode.is_atomic_inline());

    let inline_block = BoxType::InlineBlock;
    assert!(inline_block.is_inline());
    assert!(inline_block.is_inline_block());
    assert!(inline_block.is_atomic_inline());

    let mut text = BoxType::TextNode("Hello CSS".to_string());
    assert!(text.is_inline());
    assert!(text.is_text());
    assert_eq!(text.text_content(), Some("Hello CSS"));
    if let Some(t) = text.text_content_mut() {
        t.push_str(" World");
    }
    assert_eq!(text.text_content(), Some("Hello CSS World"));

    let replaced = BoxType::ReplacedElement {
        intrinsic_width: 100.0,
        intrinsic_height: 50.0,
        pixels: vec![],
    };
    assert!(!replaced.is_block());
    assert!(replaced.is_replaced());
    assert!(replaced.is_inline());
    assert!(replaced.is_atomic_inline());
    assert_eq!(replaced.intrinsic_size(), Some((100.0, 50.0)));
    assert_eq!(replaced.intrinsic_aspect_ratio(), Some(2.0));

    let iframe = BoxType::IFrame {
        src: "https://example.com".to_string(),
        srcdoc: None,
        sandbox: IFrameSandbox::default(),
        intrinsic_width: 300.0,
        intrinsic_height: 150.0,
    };
    assert!(!iframe.is_block());
    assert!(iframe.is_replaced());
    assert!(iframe.is_inline());
    assert!(iframe.is_iframe());
    assert!(iframe.is_atomic_inline());
    assert_eq!(iframe.intrinsic_size(), Some((300.0, 150.0)));
    assert_eq!(iframe.intrinsic_aspect_ratio(), Some(2.0));

    let video = BoxType::Video {
        src: "https://example.com/movie.mp4".to_string(),
        poster: None,
        has_controls: true,
        autoplay: false,
        is_loop: false,
        is_muted: false,
        is_playing: false,
        current_time: 0.0,
        duration: 120.0,
        intrinsic_width: 640.0,
        intrinsic_height: 360.0,
        poster_pixels: None,
        poster_width: 0,
        poster_height: 0,
    };
    assert!(!video.is_block());
    assert!(video.is_replaced());
    assert!(video.is_inline());
    assert!(video.is_video());
    assert!(video.is_media());
    assert!(video.is_atomic_inline());
    assert_eq!(video.intrinsic_size(), Some((640.0, 360.0)));
    assert_eq!(video.intrinsic_aspect_ratio(), Some(640.0 / 360.0));

    let audio = BoxType::Audio {
        src: "https://example.com/song.mp3".to_string(),
        has_controls: true,
        autoplay: false,
        is_loop: false,
        is_muted: false,
        is_playing: false,
        current_time: 0.0,
        duration: 180.0,
        intrinsic_width: 300.0,
        intrinsic_height: 36.0,
    };
    assert!(!audio.is_block());
    assert!(audio.is_replaced());
    assert!(audio.is_inline());
    assert!(audio.is_audio());
    assert!(audio.is_media());
    assert!(!audio.is_video());

    let canvas = BoxType::Canvas {
        canvas_key: None,
        width: 800,
        height: 600,
        intrinsic_width: 800.0,
        intrinsic_height: 600.0,
        pixels: None,
    };
    assert!(canvas.is_canvas());
    assert!(canvas.is_media());
    assert_eq!(canvas.intrinsic_size(), Some((800.0, 600.0)));
}

#[test]
fn test_iframe_sandbox_all_whatwg_flags() {
    let unconstrained = IFrameSandbox::from_attribute(None);
    assert!(!unconstrained.is_sandboxed);
    assert!(unconstrained.allow_scripts);
    assert!(unconstrained.allow_same_origin);
    assert!(unconstrained.allow_downloads);
    assert!(unconstrained.allow_pointer_lock);

    let locked = IFrameSandbox::from_attribute(Some(""));
    assert!(locked.is_sandboxed);
    assert!(!locked.allow_scripts);
    assert!(!locked.allow_same_origin);
    assert!(!locked.allow_forms);
    assert!(!locked.allow_downloads);
    assert!(!locked.allow_pointer_lock);

    let modern = IFrameSandbox::from_attribute(Some(
        "allow-scripts allow-downloads allow-pointer-lock allow-top-navigation-by-user-activation",
    ));
    assert!(modern.is_sandboxed);
    assert!(modern.allow_scripts);
    assert!(modern.allow_downloads);
    assert!(modern.allow_pointer_lock);
    assert!(modern.allow_top_navigation_by_user_activation);
    assert!(!modern.allow_top_navigation);
    assert!(!modern.allow_forms);

    let serialized = modern.to_attribute_value();
    assert!(serialized.contains("allow-scripts"));
    assert!(serialized.contains("allow-downloads"));
    assert!(serialized.contains("allow-pointer-lock"));
    assert!(serialized.contains("allow-top-navigation-by-user-activation"));
}

#[test]
fn test_margin_strut_css21_formula() {
    let mut strut = MarginStrut::new();
    assert_eq!(strut.solve(), 0.0);

    // Positive margins: max is kept
    strut.append(20.0);
    strut.append(10.0);
    strut.append(30.0);
    assert_eq!(strut.solve(), 30.0);

    // Negative margins: min is kept
    strut.append(-5.0);
    strut.append(-15.0);
    strut.append(-10.0);
    // Formula: max_pos (30.0) - abs(min_neg) (15.0) = 15.0
    assert_eq!(strut.solve(), 15.0);

    // Combined struts
    let mut strut2 = MarginStrut::new();
    strut2.append(40.0);
    strut2.append(-25.0);

    strut.append_strut(strut2);
    // max_pos = max(30, 40) = 40; min_neg = min(-15, -25) = -25.
    // solve = 40 - 25 = 15.
    assert_eq!(strut.solve(), 15.0);

    strut.reset();
    assert_eq!(strut.solve(), 0.0);
}

#[test]
fn test_logical_edge_sizes_conversion() {
    let physical = EdgeSizes::new(10.0, 20.0, 30.0, 40.0); // top=10, right=20, bottom=30, left=40

    // Horizontal-TB, LTR: inline-start=left(40), inline-end=right(20), block-start=top(10), block-end=bottom(30)
    let logical_ltr =
        LogicalEdgeSizes::from_physical(&physical, WritingMode::HorizontalTb, Direction::Ltr);
    assert_eq!(logical_ltr.inline_start, 40.0);
    assert_eq!(logical_ltr.inline_end, 20.0);
    assert_eq!(logical_ltr.block_start, 10.0);
    assert_eq!(logical_ltr.block_end, 30.0);
    assert_eq!(
        logical_ltr.to_physical(WritingMode::HorizontalTb, Direction::Ltr),
        physical
    );

    // Horizontal-TB, RTL: inline-start=right(20), inline-end=left(40)
    let logical_rtl =
        LogicalEdgeSizes::from_physical(&physical, WritingMode::HorizontalTb, Direction::Rtl);
    assert_eq!(logical_rtl.inline_start, 20.0);
    assert_eq!(logical_rtl.inline_end, 40.0);
    assert_eq!(
        logical_rtl.to_physical(WritingMode::HorizontalTb, Direction::Rtl),
        physical
    );
}

#[test]
fn test_overflow_model() {
    let border_box = Rect::new(0.0, 0.0, 200.0, 200.0);
    let mut overflow = OverflowModel::new(border_box);
    assert!(!overflow.has_scrollable_overflow(border_box));

    overflow.unite_scrollable(Rect::new(50.0, 50.0, 250.0, 100.0)); // right = 300
    assert!(overflow.has_scrollable_overflow(border_box));
    let (ext_x, ext_y) = overflow.scrollable_extent(border_box);
    assert_eq!(ext_x, 100.0);
    assert_eq!(ext_y, 0.0);
}

#[test]
fn test_resolve_replaced_size_algorithm() {
    // Both specified
    assert_eq!(
        resolve_replaced_size(
            Some(100.0),
            Some(80.0),
            Some(200.0),
            Some(150.0),
            None,
            (300.0, 150.0)
        ),
        (100.0, 80.0)
    );

    // Width specified, height auto with aspect ratio
    assert_eq!(
        resolve_replaced_size(
            Some(200.0),
            None,
            Some(400.0),
            Some(200.0),
            None,
            (300.0, 150.0)
        ),
        (200.0, 100.0)
    );

    // Height specified, width auto with aspect ratio
    assert_eq!(
        resolve_replaced_size(
            None,
            Some(100.0),
            Some(400.0),
            Some(200.0),
            None,
            (300.0, 150.0)
        ),
        (200.0, 100.0)
    );

    // Both auto, intrinsic sizes exist
    assert_eq!(
        resolve_replaced_size(None, None, Some(640.0), Some(480.0), None, (300.0, 150.0)),
        (640.0, 480.0)
    );
}

#[test]
fn test_resolve_object_fit_rect() {
    let content_box = Rect::new(0.0, 0.0, 100.0, 100.0);
    let natural = (200.0, 100.0); // 2:1 aspect ratio

    // Contain: scaled to 100x50, centered vertically at y=25
    let contain = resolve_object_fit_rect(content_box, natural, ObjectFit::Contain, (0.5, 0.5));
    assert_eq!(contain, Rect::new(0.0, 25.0, 100.0, 50.0));

    // Cover: scaled to 200x100, centered horizontally at x=-50
    let cover = resolve_object_fit_rect(content_box, natural, ObjectFit::Cover, (0.5, 0.5));
    assert_eq!(cover, Rect::new(-50.0, 0.0, 200.0, 100.0));

    // Fill: stretched to 100x100
    let fill = resolve_object_fit_rect(content_box, natural, ObjectFit::Fill, (0.5, 0.5));
    assert_eq!(fill, Rect::new(0.0, 0.0, 100.0, 100.0));
}
