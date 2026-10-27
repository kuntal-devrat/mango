use mango_core::Rect;
use mango_css::computed::ComputedStyle;
use mango_css::values::{Display, TextAlign};

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::super::container::layout_inline_children;
use super::super::shaping::{is_rtl_char, shape_and_bidi_text};


#[test]
fn test_debug_spaces() {
    let html = "<p>Predictable performance. Tiny resource footprint. Rock-solid reliability. Rust is great for network services.</p>";
    let doc = mango_html::parse_html(html);
    let st = crate::style_tree::build_style_tree(&doc, &[]).unwrap();
    let mut bt = crate::box_tree::build_box_tree(&st);
    let mut float_ctx = FloatContext::new();
    let cb = Dimensions::new(Rect::new(0.0, 0.0, 237.0, 0.0));
    crate::block_flow::layout_block(&mut bt, &cb, &mut float_ctx);
    // Call a second time to ensure multi-pass layout doesn't merge words across line boxes:
    let mut float_ctx2 = FloatContext::new();
    crate::block_flow::layout_block(&mut bt, &cb, &mut float_ctx2);

    fn collect_all_text(b: &LayoutBox, out: &mut Vec<String>) {
        if let Some(t) = b.text() {
            out.push(t.to_string());
        }
        for c in &b.children {
            collect_all_text(c, out);
        }
    }
    let mut texts = Vec::new();
    collect_all_text(&bt, &mut texts);
    let full_text = texts.join(" ");
    assert!(
        !full_text.contains("Tinyresource"),
        "Tiny and resource merged: {}",
        full_text
    );
    assert!(
        !full_text.contains("Rock-solidreliability"),
        "Rock-solid and reliability merged: {}",
        full_text
    );
    assert!(
        !full_text.contains("fornetwork"),
        "for and network merged: {}",
        full_text
    );
}

#[test]
fn test_cjk_line_wrapping() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    // 60px container fits at most ~3-4 CJK characters per line at 16px
    container.dimensions.content = Rect::new(0.0, 0.0, 60.0, 0.0);

    let mut child_style = ComputedStyle::default();
    child_style.font_size = 16.0;

    let raw = "这是一个测试，包含中文。";
    let text = LayoutBox::new(
        BoxType::TextNode(raw.to_string()),
        Some(child_style.clone().into()),
    );
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    // Must wrap across multiple lines because CJK allows character-level breaking
    assert!(
        container.children.len() >= 3,
        "CJK text without spaces must wrap into multiple lines in narrow container; got {} lines",
        container.children.len()
    );

    // Kinsoku Shori: verify closing punctuation does not start any line
    for (i, child) in container.children.iter().enumerate() {
        if let Some(t) = child.text() {
            assert!(
                !t.starts_with('，') && !t.starts_with('。'),
                "Line {} starts with closing punctuation: {:?}",
                i,
                t
            );
        }
    }

    // Now test word-break: keep-all (CJK should NOT break without spaces or punctuation)
    let mut container_keep = LayoutBox::new(BoxType::BlockNode, None);
    container_keep.dimensions.content = Rect::new(0.0, 0.0, 60.0, 0.0);
    let mut keep_style = ComputedStyle::default();
    keep_style.font_size = 16.0;
    keep_style.word_break = mango_css::values::WordBreak::KeepAll;

    let raw_unbroken = "这是一个测试包含中文";
    let text_keep = LayoutBox::new(
        BoxType::TextNode(raw_unbroken.to_string()),
        Some(keep_style.into()),
    );
    container_keep.children.push(text_keep);

    let mut float_ctx2 = FloatContext::new();
    layout_inline_children(&mut container_keep, &mut float_ctx2);
    assert_eq!(
        container_keep.children.len(),
        1,
        "word-break: keep-all must keep unbroken CJK text on a single line"
    );

    // Verify Thai combining marks (like Mai Ek '\u{0E48}' and Sara I '\u{0E34}') stay attached to consonants
    let mut container_thai = LayoutBox::new(BoxType::BlockNode, None);
    container_thai.dimensions.content = Rect::new(0.0, 0.0, 30.0, 0.0);
    let thai_raw = "ที่นี่"; // "thi-ni" (at this place): Consonant 'ท' + Sara I + Mai Ek, Consonant 'น' + Sara I + Mai Ek
    let text_thai = LayoutBox::new(
        BoxType::TextNode(thai_raw.to_string()),
        Some(child_style.into()),
    );
    container_thai.children.push(text_thai);
    let mut float_ctx3 = FloatContext::new();
    layout_inline_children(&mut container_thai, &mut float_ctx3);
    for child in &container_thai.children {
        if let Some(t) = child.text() {
            assert!(
                !t.starts_with('\u{0E34}') && !t.starts_with('\u{0E48}'),
                "Combining mark cannot start a line by itself: {:?}",
                t
            );
        }
    }
}

#[test]
fn test_rtl_shaping_and_bidi() {
    // Test Arabic cursive shaping and Lam-Alef ligature for "سلام" (Seen, Lam, Alef, Meem)
    let salam = "سلام";
    let shaped_salam = shape_and_bidi_text(salam);
    // Must contain Final Lam-Alef ligature (\u{FEFC}) and Initial Seen (\u{FEB3})
    assert!(
        shaped_salam.contains('\u{FEFC}'),
        "Shaped 'سلام' must contain Lam-Alef ligature \\u{{FEFC}}, got: {:?}",
        shaped_salam
    );
    assert!(
        shaped_salam.contains('\u{FEB3}'),
        "Shaped 'سلام' must contain Initial Seen \\u{{FEB3}}, got: {:?}",
        shaped_salam
    );

    // Test multi-word Arabic Bidi ordering: "مرحبا بكم"
    let phrase = "مرحبا بكم";
    let shaped_phrase = shape_and_bidi_text(phrase);
    // In visual LTR blitting, the second word "بكم" is drawn on the left, and "مرحبا" on the right!
    let words: Vec<&str> = shaped_phrase.split_whitespace().collect();
    assert_eq!(words.len(), 2, "Shaped phrase should have 2 words");

    // Verify both visual words are Arabic
    assert!(
        words[0].chars().any(is_rtl_char),
        "Visual word 0 should be Arabic"
    );
    assert!(
        words[1].chars().any(is_rtl_char),
        "Visual word 1 should be Arabic"
    );

    // Test mixed LTR + RTL text: "Hello سلام World"
    let mixed = "Hello سلام World";
    let shaped_mixed = shape_and_bidi_text(mixed);
    assert!(
        shaped_mixed.starts_with("Hello "),
        "Mixed text must start with LTR 'Hello '"
    );
    assert!(
        shaped_mixed.ends_with(" World"),
        "Mixed text must end with LTR ' World'"
    );
    assert!(
        shaped_mixed.contains('\u{FEFC}'),
        "Mixed text must contain shaped Arabic ligature"
    );
}

#[test]
fn test_text_align_justify_space_expansion() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 200.0, 0.0);
    let mut style = ComputedStyle::default();
    style.font_size = 16.0;
    style.text_align = TextAlign::Justify;
    container.style = Some(style.clone().into());

    // "Hello world from mango browser engine" - long enough to wrap across 2 lines
    let text = LayoutBox::new(
        BoxType::TextNode("Hello world from mango browser engine".to_string()),
        Some(style.into()),
    );
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert!(
        container.children.len() >= 2,
        "Expected multiple lines, got {}",
        container.children.len()
    );
    // The first line should be justified: start at 0.0 and span the full container width
    let first_line = &container.children[0];
    assert_eq!(first_line.dimensions.content.origin.x, 0.0);
    let first_line_w = first_line.dimensions.content.width();
    assert!(
        (first_line_w - 200.0).abs() < 5.0,
        "First line of justified text should span ~200px, got {}",
        first_line_w
    );
}

#[test]
fn test_inline_node_horizontal_spacing() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 400.0, 0.0);
    let mut cont_style = ComputedStyle::default();
    cont_style.font_size = 16.0;
    container.style = Some(cont_style.clone().into());

    // Inline span with margin-left: 20px, padding-left: 10px
    let mut span_style = ComputedStyle::default();
    span_style.font_size = 16.0;
    span_style.margin_left = mango_css::values::Length::Px(20.0);
    span_style.padding_left = mango_css::values::Length::Px(10.0);

    let mut span = LayoutBox::new(BoxType::InlineNode, Some(span_style.into()));
    let text = LayoutBox::new(
        BoxType::TextNode("Inside Span".to_string()),
        Some(cont_style.clone().into()),
    );
    span.children.push(text);
    container.children.push(span);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert!(!container.children.is_empty());
    let child_x = container.children[0].dimensions.content.origin.x;
    assert!(
        (child_x - 30.0).abs() < 0.1,
        "Text inside span should be shifted by 30px (margin + padding), got {}",
        child_x
    );
}

#[test]
fn test_whitespace_only_line_height_not_collapsed() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 400.0, 0.0);
    let mut style = ComputedStyle::default();
    style.font_size = 20.0;
    style.white_space = mango_css::values::WhiteSpace::Pre;
    container.style = Some(style.clone().into());

    // TextNode containing only a space
    let text = LayoutBox::new(BoxType::TextNode("   ".to_string()), Some(style.into()));
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    let h = layout_inline_children(&mut container, &mut float_ctx);

    // Height should be based on font metrics (~24px for 20px font), not collapsed to 1.0px
    assert!(
        h >= 16.0,
        "Whitespace line should retain font line-height, got {}",
        h
    );
}

#[test]
fn test_rtl_text_indent_start_edge() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 300.0, 0.0);
    let mut style = ComputedStyle::default();
    style.font_size = 16.0;
    style.direction = mango_css::values::Direction::Rtl;
    style.text_indent = mango_css::values::Length::Px(40.0);
    container.style = Some(style.clone().into());

    let text = LayoutBox::new(BoxType::TextNode("مرحبا".to_string()), Some(style.into()));
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert_eq!(container.children.len(), 1);
    let child = &container.children[0];
    let child_right = child.dimensions.content.origin.x + child.dimensions.content.width();
    // In 300px container with 40px text-indent in RTL, right edge should be at 300 - 40 = 260px
    assert!(
        (child_right - 260.0).abs() < 1.0,
        "RTL text with text-indent: 40px should end at 260px, got {}",
        child_right
    );
}

#[test]
fn test_inline_block_baseline_alignment() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 400.0, 0.0);
    let mut style = ComputedStyle::default();
    style.font_size = 16.0;
    container.style = Some(style.clone().into());

    // Text 1
    let text1 = LayoutBox::new(
        BoxType::TextNode("Hello ".to_string()),
        Some(style.clone().into()),
    );

    // Inline-block containing Text 2 with top padding 10px
    let mut ib_box = LayoutBox::new(BoxType::InlineBlock, None);
    let mut ib_style = ComputedStyle::default();
    ib_style.display = Display::InlineBlock;
    ib_style.font_size = 16.0;
    ib_style.padding_top = mango_css::values::Length::Px(10.0);
    ib_style.padding_bottom = mango_css::values::Length::Px(10.0);
    ib_box.style = Some(ib_style.clone().into());

    let text2 = LayoutBox::new(
        BoxType::TextNode("Button".to_string()),
        Some(ib_style.into()),
    );
    ib_box.children.push(text2);

    container.children.push(text1);
    container.children.push(ib_box);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    // Child 0 is TextNode "Hello ", Child 1 is InlineBlock
    assert_eq!(container.children.len(), 2);
    let text_child = &container.children[0];
    let ib_child = &container.children[1];

    // The text inside the inline block is child 0 of ib_child
    assert_eq!(ib_child.children.len(), 1);
    let inner_text = &ib_child.children[0];

    // In CSS 2.1 §10.8.1, the inline-block baseline aligns with the line baseline.
    // Therefore, the text inside the inline-block and the surrounding text have matching baseline Y coordinates.
    let text1_baseline = text_child.dimensions.content.y() + (16.0 * 0.8);
    let text2_baseline = inner_text.dimensions.content.y() + (16.0 * 0.8);
    assert!(
        (text1_baseline - text2_baseline).abs() < 2.0,
        "Inline-block inner text baseline ({}) should align with line baseline ({})",
        text2_baseline,
        text1_baseline
    );
}

#[test]
fn test_text_overflow_ellipsis_truncation() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 100.0, 0.0);

    let mut style = ComputedStyle::default();
    style.font_size = 16.0;
    style.white_space = mango_css::values::WhiteSpace::Nowrap;
    style.text_overflow = mango_css::values::TextOverflow::Ellipsis;
    container.style = Some(style.clone().into());

    let long_text = LayoutBox::new(
        BoxType::TextNode(
            "This is a very long sentence that must be truncated with ellipsis".to_string(),
        ),
        Some(style.into()),
    );
    container.children.push(long_text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert!(!container.children.is_empty());
    let child = &container.children[0];
    if let BoxType::TextNode(ref t) = child.box_type {
        assert!(
            t.ends_with('…'),
            "Truncated text must end with ellipsis '…', got: {}",
            t
        );
        assert!(
            child.dimensions.content.width() <= 100.0,
            "Truncated text width ({}) must not exceed container available width (100.0)",
            child.dimensions.content.width()
        );
    } else {
        panic!("Expected TextNode child");
    }
}

#[test]
fn test_text_wrap_balance_and_pretty() {
    // Test 1: text-wrap: balance balances lines
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 300.0, 0.0);

    let mut style = ComputedStyle::default();
    style.font_size = 16.0;
    style.text_wrap = mango_css::values::TextWrap::Balance;
    container.style = Some(style.clone().into());

    let text = LayoutBox::new(
        BoxType::TextNode(
            "Short heading that wraps across multiple balanced lines cleanly".to_string(),
        ),
        Some(style.into()),
    );
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    // There should be lines produced
    assert!(container.children.len() >= 2);
    // Measure line boxes Y coordinates to group them by lines
    let mut line_ys = Vec::new();
    for child in &container.children {
        let y = child.dimensions.content.y();
        if !line_ys.iter().any(|&prev_y: &f32| (prev_y - y).abs() < 1.0) {
            line_ys.push(y);
        }
    }
    assert!(line_ys.len() >= 2, "Expected at least 2 lines");

    // Test 2: text-wrap: pretty prevents single-word orphans
    let mut pretty_container = LayoutBox::new(BoxType::BlockNode, None);
    pretty_container.dimensions.content = Rect::new(0.0, 0.0, 220.0, 0.0);

    let mut pretty_style = ComputedStyle::default();
    pretty_style.font_size = 16.0;
    pretty_style.text_wrap = mango_css::values::TextWrap::Pretty;
    pretty_container.style = Some(pretty_style.clone().into());

    let pretty_text = LayoutBox::new(
        BoxType::TextNode("Quick brown fox jumps over dog".to_string()),
        Some(pretty_style.into()),
    );
    pretty_container.children.push(pretty_text);

    let mut float_ctx2 = FloatContext::new();
    layout_inline_children(&mut pretty_container, &mut float_ctx2);

    // Group children by line Y
    let mut lines: std::collections::BTreeMap<i32, Vec<String>> = std::collections::BTreeMap::new();
    for child in &pretty_container.children {
        if let BoxType::TextNode(ref t) = child.box_type {
            let y_key = (child.dimensions.content.y() * 10.0).round() as i32;
            lines.entry(y_key).or_default().push(t.clone());
        }
    }
    if lines.len() >= 2 {
        let last_line_words = lines.values().last().unwrap();
        let non_empty_words: Vec<_> = last_line_words
            .iter()
            .filter(|w| !w.trim().is_empty())
            .collect();
        assert!(
            non_empty_words.len() >= 2 || last_line_words.join(" ").split_whitespace().count() >= 2,
            "Last line should have at least 2 words with text-wrap: pretty (prevent orphan), got: {:?}",
            last_line_words
        );
    }
}
