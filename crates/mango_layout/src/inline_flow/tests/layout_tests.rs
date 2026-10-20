use mango_core::Rect;
use mango_css::computed::ComputedStyle;

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::float::FloatContext;

use super::super::container::layout_inline_children;
use super::super::metrics::measure_text_width;

#[test]
fn test_explicit_line_height_defines_block_height() {
    // A single line of 40px text with `line-height: 40px` must produce a 40px
    // tall block. Chromium lets the glyphs overflow the line box rather than
    // growing the block, and matching that keeps stacked blocks aligned.
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 800.0, 0.0);

    let mut style = ComputedStyle::default();
    style.font_size = 40.0;
    style.font_family = "Arial, sans-serif".to_string();
    style.line_height = Some(40.0);

    container.children.push(LayoutBox::new(
        BoxType::TextNode("H1".to_string()),
        Some(style.into()),
    ));

    let mut float_ctx = FloatContext::new();
    let height = layout_inline_children(&mut container, &mut float_ctx);
    assert!(
        (height - 40.0).abs() < 0.01,
        "line-height 40px must yield a 40px block, got {height}"
    );
}

#[test]
fn test_negative_half_leading_not_clamped() {
    // `line-height` below the font's natural height produces negative half-leading,
    // which lifts the text above the line box (CSS 2.1 §10.8). Clamping it to zero
    // pushes tight-line-height text down by half the shortfall.
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 800.0, 0.0);

    let mut style = ComputedStyle::default();
    style.font_size = 40.0;
    style.font_family = "Arial, sans-serif".to_string();
    style.line_height = Some(40.0);

    container.children.push(LayoutBox::new(
        BoxType::TextNode("H1".to_string()),
        Some(style.into()),
    ));

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    let run = &container.children[0];
    let text_top = run.dimensions.content.y();
    assert!(
        text_top < -1.0,
        "tight line-height must lift the run above the line box top, got {text_top}"
    );
}

#[test]
fn test_normal_line_height_uses_font_metrics() {
    // `line-height: normal` is the font's own ascent + descent + line gap, not a
    // fixed 1.3x multiplier, which would make every wrapped paragraph taller
    // than Chromium's.
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 800.0, 0.0);

    let mut style = ComputedStyle::default();
    style.font_size = 16.0;
    style.font_family = "Arial, sans-serif".to_string();

    container.children.push(LayoutBox::new(
        BoxType::TextNode("one line".to_string()),
        Some(style.into()),
    ));

    let mut float_ctx = FloatContext::new();
    let height = layout_inline_children(&mut container, &mut float_ctx);
    let (ascent, descent, gap) = mango_render::font::font_manager().font_metrics(
        mango_render::FontFamily::SansSerif,
        mango_render::FontWeight::Regular,
        16.0,
    );
    let expected = ascent + descent + gap;
    assert!(
        (height - expected).abs() < 0.01,
        "normal line-height must equal ascent + descent + gap ({expected}), got {height}"
    );
}

#[test]
fn test_measure_text_width() {
    let w = measure_text_width("Hello", 16.0);
    assert!(w > 0.0);
    let w2 = measure_text_width("Hello World", 16.0);
    assert!(w2 > w);
}

#[test]
fn test_layout_inline_children_wrapping() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 60.0, 0.0);

    let mut child_style = ComputedStyle::default();
    child_style.font_size = 16.0;

    let text = LayoutBox::new(
        BoxType::TextNode("Hello World Foo Bar".to_string()),
        Some(child_style.into()),
    );
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    let height = layout_inline_children(&mut container, &mut float_ctx);

    assert!(height > 0.0);
    assert!(
        container.children.len() > 1,
        "Expected multiple line boxes, got {}",
        container.children.len()
    );
}

#[test]
fn test_heading_words_do_not_overlap() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 500.0, 0.0);

    let mut child_style = ComputedStyle::default();
    child_style.font_size = 32.0;

    let text = LayoutBox::new(
        BoxType::TextNode("Heading With Multiple Words".to_string()),
        Some(child_style.into()),
    );
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    for i in 0..container.children.len().saturating_sub(1) {
        let cur = &container.children[i];
        let next = &container.children[i + 1];
        if (cur.dimensions.content.y() - next.dimensions.content.y()).abs() < 1.0 {
            assert!(
                cur.dimensions.content.right() <= next.dimensions.content.x() + 0.1,
                "Words on the same line must not overlap: {:?} and {:?}",
                cur.dimensions.content,
                next.dimensions.content
            );
        }
    }
}

#[test]
fn test_letter_spacing_expands_text_width() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 500.0, 0.0);

    let mut base_style = ComputedStyle::default();
    base_style.font_size = 16.0;

    let mut spaced_style = ComputedStyle::default();
    spaced_style.font_size = 16.0;
    spaced_style.letter_spacing = mango_css::values::Length::Px(5.0);

    let text1 = LayoutBox::new(
        BoxType::TextNode("SPACED".to_string()),
        Some(base_style.into()),
    );
    let text2 = LayoutBox::new(
        BoxType::TextNode("SPACED".to_string()),
        Some(spaced_style.into()),
    );

    let mut c1 = container.clone();
    c1.children.push(text1);
    let mut c2 = container;
    c2.children.push(text2);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut c1, &mut float_ctx);
    layout_inline_children(&mut c2, &mut float_ctx);

    let w1 = c1.children[0].dimensions.content.width();
    let w2 = c2.children[0].dimensions.content.width();
    assert!(
        w2 > w1,
        "Letter spaced text must be wider: {w2} should be > {w1}"
    );
}

#[test]
fn test_text_indent_offsets_first_line() {
    let mut container_style = ComputedStyle::default();
    container_style.text_indent = mango_css::values::Length::Px(40.0);
    let mut container = LayoutBox::new(BoxType::BlockNode, Some(container_style.into()));
    container.dimensions.content = Rect::new(0.0, 0.0, 200.0, 0.0);

    let mut child_style = ComputedStyle::default();
    child_style.font_size = 16.0;

    let text = LayoutBox::new(
        BoxType::TextNode(
            "Line one text that is quite long so it definitely wraps into two lines.".to_string(),
        ),
        Some(child_style.into()),
    );
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert!(
        container.children.len() >= 2,
        "Text must wrap into multiple lines"
    );
    let first_line_x = container.children[0].dimensions.content.x();
    assert_eq!(
        first_line_x, 40.0,
        "First line must start at text-indent (40px)"
    );

    let second_line_box = container
        .children
        .iter()
        .find(|b| b.dimensions.content.y() > container.children[0].dimensions.content.y());
    assert!(second_line_box.is_some());
    assert_eq!(
        second_line_box.unwrap().dimensions.content.x(),
        0.0,
        "Second line must not have text-indent"
    );
}

#[test]
fn test_vertical_align_super_and_middle() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 500.0, 0.0);

    let mut normal_style = ComputedStyle::default();
    normal_style.font_size = 16.0;
    let normal_child = LayoutBox::new(
        BoxType::TextNode("Normal text".to_string()),
        Some(normal_style.into()),
    );

    let mut super_style = ComputedStyle::default();
    super_style.font_size = 12.0;
    super_style.vertical_align = mango_css::values::VerticalAlign::Super;
    let super_child = LayoutBox::new(
        BoxType::TextNode("[1]".to_string()),
        Some(super_style.into()),
    );

    container.children.push(normal_child);
    container.children.push(super_child);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert_eq!(
        container.children.len(),
        2,
        "Normal and super text runs should remain separate"
    );
    let normal_y = container.children[0].dimensions.content.y();
    let super_y = container.children[1].dimensions.content.y();
    assert!(
        super_y < normal_y,
        "Superscript text must be raised above normal baseline (super_y={super_y} < normal_y={normal_y})"
    );
}

#[test]
fn test_word_break_opportunity_wbr_and_zero_width_space() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 100.0, 0.0);

    let mut style = ComputedStyle::default();
    style.font_size = 16.0;

    let mut inline_node = LayoutBox::new(BoxType::InlineNode, Some(style.clone().into()));
    inline_node.children.push(LayoutBox::new(
        BoxType::TextNode("VeryLongWord".to_string()),
        Some(style.clone().into()),
    ));

    let mut wbr_box = LayoutBox::new(BoxType::InlineNode, Some(style.clone().into()));
    wbr_box.tag_name = Some("wbr".to_string());
    inline_node.children.push(wbr_box);

    inline_node.children.push(LayoutBox::new(
        BoxType::TextNode("ThatWraps".to_string()),
        Some(style.into()),
    ));

    container.children.push(inline_node);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert!(
        container.children.len() >= 2,
        "<wbr> must allow breaking long words across lines"
    );
    let y0 = container.children[0].dimensions.content.y();
    let y1 = container.children[1].dimensions.content.y();
    assert!(y1 > y0, "Second part must wrap to next line");
}

#[test]
fn test_bidi_override_visual_reversal() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 500.0, 0.0);

    let mut style = ComputedStyle::default();
    style.font_size = 16.0;
    style.direction = mango_css::values::Direction::Rtl;
    style.unicode_bidi = mango_css::values::UnicodeBidi::BidiOverride;

    let text = LayoutBox::new(BoxType::TextNode("ABCDEF".to_string()), Some(style.into()));
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert_eq!(container.children.len(), 1);
    let rendered_text = container.children[0].text().unwrap();
    assert_eq!(
        rendered_text, "FEDCBA",
        "bidi-override with RTL direction must visually reverse text"
    );
}

#[test]
fn test_word_break_break_all() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 80.0, 0.0);

    let mut child_style = ComputedStyle::default();
    child_style.font_size = 16.0;
    child_style.word_break = mango_css::values::WordBreak::BreakAll;

    let text = LayoutBox::new(
        BoxType::TextNode("Supercalifragilisticexpialidocious".to_string()),
        Some(child_style.into()),
    );
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert!(
        container.children.len() > 1,
        "word-break: break-all must break long single word across lines"
    );
}

#[test]
fn test_overflow_wrap_break_word() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 70.0, 0.0);

    let mut child_style = ComputedStyle::default();
    child_style.font_size = 16.0;
    child_style.overflow_wrap = mango_css::values::OverflowWrap::BreakWord;

    let text = LayoutBox::new(
        BoxType::TextNode("Incomprehensibilities".to_string()),
        Some(child_style.into()),
    );
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert!(
        container.children.len() > 1,
        "overflow-wrap: break-word must break word when exceeding line width"
    );
}

#[test]
fn test_hyphens_auto() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 90.0, 0.0);

    let mut child_style = ComputedStyle::default();
    child_style.font_size = 16.0;
    child_style.hyphens = mango_css::values::Hyphens::Auto;

    let text = LayoutBox::new(
        BoxType::TextNode("Internationalization".to_string()),
        Some(child_style.into()),
    );
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert!(
        container.children.len() > 1,
        "hyphens: auto must hyphenate long word across lines"
    );
    let first_text = container.children[0].text().unwrap();
    assert!(
        first_text.ends_with('-'),
        "hyphenated prefix must end with hyphen '-': got {}",
        first_text
    );
}

#[test]
fn test_soft_hyphen() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 70.0, 0.0);

    let mut child_style = ComputedStyle::default();
    child_style.font_size = 16.0;
    child_style.hyphens = mango_css::values::Hyphens::Manual;

    let text = LayoutBox::new(
        BoxType::TextNode("hy\u{00AD}phen\u{00AD}ation".to_string()),
        Some(child_style.into()),
    );
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert!(
        container.children.len() > 1,
        "Soft hyphen must wrap word across lines"
    );
    let first_text = container.children[0].text().unwrap();
    assert!(
        first_text.ends_with('-'),
        "Soft hyphen break must insert visible hyphen: got {}",
        first_text
    );
}

#[test]
fn test_line_clamp() {
    let mut container_style = ComputedStyle::default();
    container_style.line_clamp = mango_css::values::LineClamp::Lines(2);
    let mut container = LayoutBox::new(BoxType::BlockNode, Some(container_style.into()));
    container.dimensions.content = Rect::new(0.0, 0.0, 100.0, 0.0);

    let mut child_style = ComputedStyle::default();
    child_style.font_size = 16.0;
    child_style.line_clamp = mango_css::values::LineClamp::Lines(2);

    let text = LayoutBox::new(
        BoxType::TextNode("Line one text that wraps easily. Line two text that continues. Line three text that should be clamped.".to_string()),
        Some(child_style.into()),
    );
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert!(
        container.children.len() <= 2,
        "line-clamp: 2 must clamp children to at most 2 boxes"
    );
}

#[test]
fn test_rust_text_layout() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 360.0, 0.0);

    let mut child_style = ComputedStyle::default();
    child_style.font_size = 18.0;

    let raw = "Rust’s rich type system and ownership model guarantee memory-safety\nand thread-safety — enabling you to eliminate many classes of\nbugs at compile-time.";
    let collapsed = crate::style_tree::collapse_whitespace(raw);
    let text = LayoutBox::new(BoxType::TextNode(collapsed), Some(child_style.into()));
    container.children.push(text);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    for (i, child) in container.children.iter().enumerate() {
        println!("Line {}: {:?}", i, child.text());
    }
}
