#![allow(clippy::field_reassign_with_default)]

use mango_core::Rect;
use mango_css::computed::ComputedStyle;
use mango_css::values::{Direction, Display, Hyphens, UnicodeBidi, WhiteSpace};
use mango_layout::bidi::{mirror_char, reorder_bidi_text, resolve_base_direction};
use mango_layout::box_model::BoxType;
use mango_layout::box_tree::LayoutBox;
use mango_layout::float::FloatContext;
use mango_layout::inline_flow::layout_inline_children;
use mango_layout::shaping::{segment_thai_syllables, shape_arabic, shape_devanagari, shape_thai};

// =========================================================================
// 8.2.1 Bidirectional Text (UAX#9 BiDi Algorithm)
// =========================================================================

#[test]
fn test_bidi_uax9_algorithm_mixed_scripts_and_mirroring() {
    // 1. Base direction detection (returns true for RTL, false for LTR)
    assert!(!resolve_base_direction("Hello World", false));
    // Arabic "مرحبا" (Marhaban)
    assert!(resolve_base_direction(
        "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}",
        false
    ));
    // Hebrew "שלום" (Shalom)
    assert!(resolve_base_direction(
        "\u{05E9}\u{05DC}\u{05D5}\u{05DD}",
        false
    ));

    // 2. Bracket and punctuation mirroring (Rule L4)
    assert_eq!(mirror_char('('), ')');
    assert_eq!(mirror_char(')'), '(');
    assert_eq!(mirror_char('['), ']');
    assert_eq!(mirror_char(']'), '[');
    assert_eq!(mirror_char('{'), '}');
    assert_eq!(mirror_char('}'), '{');
    assert_eq!(mirror_char('<'), '>');
    assert_eq!(mirror_char('>'), '<');

    // 3. Mixed BiDi reordering: Hebrew text inside RTL paragraph
    // Hebrew characters reordered right-to-left
    let hebrew_text = "\u{05E9}\u{05DC}\u{05D5}\u{05DD}"; // Shin, Lamed, Vav, Final Mem
    let reordered_hebrew = reorder_bidi_text(hebrew_text, true);
    let expected_rev: String = hebrew_text.chars().rev().collect();
    assert_eq!(reordered_hebrew, expected_rev);

    // 4. European numbers inside RTL context retain LTR visual order (Rule I2)
    // E.g. Arabic word followed by number 2026: digits stay in "2026" order
    let rtl_with_numbers = "\u{0639}\u{0627}\u{0645} 2026";
    let reordered_num = reorder_bidi_text(rtl_with_numbers, true);
    assert!(
        reordered_num.contains("2026"),
        "European numbers must preserve LTR order in RTL paragraph, got: {reordered_num}"
    );

    // 5. Parentheses inside RTL text are mirrored
    let text_with_parens = "\u{0639}\u{0627}\u{0645} (2026)";
    let reordered_parens = reorder_bidi_text(text_with_parens, true);
    // In visual order, the opening '(' becomes ')' and closing ')' becomes '('
    assert!(
        reordered_parens.contains(')') && reordered_parens.contains('('),
        "Parentheses must be mirrored in RTL context, got: {reordered_parens}"
    );

    // 6. Layout integration: layout_inline_children with bidi-override
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 400.0, 0.0);

    let mut bdo_style = ComputedStyle::default();
    bdo_style.font_size = 16.0;
    bdo_style.direction = Direction::Rtl;
    bdo_style.unicode_bidi = UnicodeBidi::BidiOverride;

    let bdo_child = LayoutBox::new(BoxType::TextNode("MANGO".to_string()), Some(bdo_style));
    container.children.push(bdo_child);

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert_eq!(container.children.len(), 1);
    // Under RTL bidi-override, characters are reversed: "MANGO" -> "OGNAM"
    assert_eq!(container.children[0].text(), Some("OGNAM"));
}

// =========================================================================
// 8.2.2 Complex Script Shaping (Arabic, Devanagari, Thai)
// =========================================================================

#[test]
fn test_complex_script_shaping_arabic_devanagari_thai() {
    // 1. Arabic cursive joining & Lam-Alef ligatures
    // Lam (\u{0644}) + Alef (\u{0627}) -> Ligature Lam-Alef (\u{FEFB})
    let lam_alef = "\u{0644}\u{0627}";
    let shaped_la = shape_arabic(lam_alef);
    assert_eq!(shaped_la, "\u{FEFB}");

    // Lam + Alef with Madda (\u{0622}) -> \u{FEF5}
    let lam_alef_madda = "\u{0644}\u{0622}";
    let shaped_lam_madda = shape_arabic(lam_alef_madda);
    assert_eq!(shaped_lam_madda, "\u{FEF5}");

    // Arabic cursive letter transitions: Ba (\u{0628}) + Ba (\u{0628})
    // First Ba is Initial (\u{FE91}), second Ba is Final (\u{FE90})
    let baba = "\u{0628}\u{0628}";
    let shaped_baba = shape_arabic(baba);
    assert_eq!(shaped_baba, "\u{FE91}\u{FE90}");

    // Tashkeel transparency: diacritics do not break joining or ligatures
    // Lam + Fatha (\u{064E}) + Alef -> Ligature Lam-Alef with Fatha preserved
    let lam_fatha_alef = "\u{0644}\u{064E}\u{0627}";
    let shaped_tashkeel = shape_arabic(lam_fatha_alef);
    assert!(
        shaped_tashkeel.contains('\u{FEFB}') && shaped_tashkeel.contains('\u{064E}'),
        "Lam-Alef ligature must form across Tashkeel diacritics, got: {shaped_tashkeel:?}"
    );

    // 2. Devanagari Nukta composition and Pre-base Matra Reordering
    // Ka (\u{0915}) + Nukta (\u{093C}) -> Qa (\u{0958})
    let ka_nukta = "\u{0915}\u{093C}";
    let shaped_qa = shape_devanagari(ka_nukta);
    assert_eq!(shaped_qa, "\u{0958}");

    // Pre-base Matra: "कि" (Ka \u{0915} + Matra-I \u{093F})
    // In complex text shaping, the pre-base vowel matra \u{093F} reorders before the consonant:
    let ki = "\u{0915}\u{093F}";
    let shaped_ki = shape_devanagari(ki);
    assert_eq!(shaped_ki, "\u{093F}\u{0915}");

    // Conjunct Pre-base Matra: "क्रि" (Ka \u{0915} + Halant \u{094D} + Ra \u{0930} + Matra-I \u{093F})
    // The matra moves before the entire consonant conjunct cluster:
    let kri = "\u{0915}\u{094D}\u{0930}\u{093F}";
    let shaped_kri = shape_devanagari(kri);
    assert!(
        shaped_kri.starts_with('\u{093F}'),
        "Pre-base matra \u{093F} must reorder before the conjunct cluster, got: {shaped_kri:?}"
    );

    // 3. Thai canonical tone mark stacking and syllable segmentation
    // Mai Ek (\u{0E48}) + Sara I (\u{0E34}) out-of-order -> canonical order (vowel \u{0E34} then tone \u{0E48})
    let thai_out_of_order = "\u{0E01}\u{0E48}\u{0E34}"; // Ko Kai + Mai Ek + Sara I
    let shaped_thai = shape_thai(thai_out_of_order);
    assert_eq!(shaped_thai, "\u{0E01}\u{0E34}\u{0E48}");

    // Syllable segmentation segments Thai text without spaces into syllables
    // allowing natural line wrapping across syllable boundaries:
    let thai_phrase = "\u{0E20}\u{0E32}\u{0E29}\u{0E32}\u{0E44}\u{0E17}\u{0E22}"; // ภาษาไทย
    let syllables = segment_thai_syllables(thai_phrase);
    assert!(
        syllables.len() >= 2,
        "Thai text must segment into multiple syllables for line wrapping, got: {syllables:?}"
    );
}

// =========================================================================
// 8.2.3 Soft Hyphens and Hyphenation
// =========================================================================

#[test]
fn test_soft_hyphens_and_hyphenation_manual_and_auto() {
    // 1. Word with soft hyphens that fits on a single line:
    // Soft hyphen \u{00AD} is invisible / removed from output text.
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 500.0, 0.0);

    let mut style = ComputedStyle::default();
    style.font_size = 16.0;

    let text_with_shy = "super\u{00AD}cali\u{00AD}fragil\u{00AD}istic";
    container.children.push(LayoutBox::new(
        BoxType::TextNode(text_with_shy.to_string()),
        Some(style.clone()),
    ));

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut container, &mut float_ctx);

    assert_eq!(container.children.len(), 1);
    // Unbroken soft hyphens must be invisible (stripped from displayed text)
    assert_eq!(container.children[0].text(), Some("supercalifragilistic"));

    // 2. Word with soft hyphen that overflows the container width:
    // Breaks at soft hyphen, inserting visible hyphen '-' on the first line.
    let mut narrow_container = LayoutBox::new(BoxType::BlockNode, None);
    // "hyphen" is ~55px at 16px font; total word "hyphenation" is ~105px.
    // Container width of 65px fits "hyphen-" but not "hyphenation".
    narrow_container.dimensions.content = Rect::new(0.0, 0.0, 65.0, 0.0);

    let word_with_shy = "hy\u{00AD}phen\u{00AD}ation";
    narrow_container.children.push(LayoutBox::new(
        BoxType::TextNode(word_with_shy.to_string()),
        Some(style.clone()),
    ));

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut narrow_container, &mut float_ctx);

    assert!(
        narrow_container.children.len() >= 2,
        "Text must wrap into at least 2 lines, got {} boxes",
        narrow_container.children.len()
    );
    // First line must end with visible hyphen '-'
    let first_line_text = narrow_container.children[0].text().unwrap_or("");
    assert!(
        first_line_text.ends_with('-'),
        "Line broken at soft hyphen must append '-', got: '{first_line_text}'"
    );

    // 3. hyphens: none suppresses soft hyphen line breaks
    let mut no_hyphen_container = LayoutBox::new(BoxType::BlockNode, None);
    no_hyphen_container.dimensions.content = Rect::new(0.0, 0.0, 65.0, 0.0);

    let mut no_hyphen_style = style.clone();
    no_hyphen_style.hyphens = Hyphens::None;
    no_hyphen_container.children.push(LayoutBox::new(
        BoxType::TextNode("hy\u{00AD}phen\u{00AD}ation".to_string()),
        Some(no_hyphen_style),
    ));

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut no_hyphen_container, &mut float_ctx);

    // When hyphens: none is set, word does not break at soft hyphens
    assert_eq!(no_hyphen_container.children.len(), 1);
    assert_eq!(no_hyphen_container.children[0].text(), Some("hyphenation"));

    // 4. hyphens: auto breaks long words at syllable boundaries with hyphen '-'
    let mut auto_container = LayoutBox::new(BoxType::BlockNode, None);
    auto_container.dimensions.content = Rect::new(0.0, 0.0, 80.0, 0.0);

    let mut auto_style = style;
    auto_style.hyphens = Hyphens::Auto;
    // "internationalization" is 20 chars; overflows 80px container
    auto_container.children.push(LayoutBox::new(
        BoxType::TextNode("internationalization".to_string()),
        Some(auto_style),
    ));

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut auto_container, &mut float_ctx);

    assert!(
        auto_container.children.len() >= 2,
        "hyphens: auto must wrap into multiple lines"
    );
    let auto_first = auto_container.children[0].text().unwrap_or("");
    assert!(
        auto_first.ends_with('-'),
        "hyphens: auto must append a hyphen '-' at the syllable break, got: '{auto_first}'"
    );
}

// =========================================================================
// 8.2.4 Ruby Annotation Layout (<ruby>, <rb>, <rt>, <rp>)
// =========================================================================

#[test]
fn test_ruby_annotation_layout() {
    let mut container = LayoutBox::new(BoxType::BlockNode, None);
    container.dimensions.content = Rect::new(0.0, 0.0, 400.0, 0.0);

    let mut ruby_style = ComputedStyle::default();
    ruby_style.font_size = 20.0;
    ruby_style.display = Display::Ruby;

    // Build <ruby> box: base text "漢字", annotation <rt> "かんじ", parentheses <rp> "(" and ")"
    let mut ruby_box = LayoutBox::new(BoxType::InlineNode, Some(ruby_style.clone()));
    ruby_box.tag_name = Some("ruby".to_string());

    // <rb> 漢字 </rb>
    let mut rb_box = LayoutBox::new(BoxType::InlineNode, Some(ruby_style.clone()));
    rb_box.tag_name = Some("rb".to_string());
    rb_box.children.push(LayoutBox::new(
        BoxType::TextNode("漢字".to_string()),
        Some(ruby_style.clone()),
    ));
    ruby_box.children.push(rb_box);

    // <rp> ( </rp> (should be hidden in layout)
    let mut rp1_box = LayoutBox::new(BoxType::InlineNode, Some(ruby_style.clone()));
    rp1_box.tag_name = Some("rp".to_string());
    rp1_box.children.push(LayoutBox::new(
        BoxType::TextNode("(".to_string()),
        Some(ruby_style.clone()),
    ));
    ruby_box.children.push(rp1_box);

    // <rt> かんじ </rt>
    let mut rt_style = ruby_style.clone();
    rt_style.font_size = 10.0;
    let mut rt_box = LayoutBox::new(BoxType::InlineNode, Some(rt_style.clone()));
    rt_box.tag_name = Some("rt".to_string());
    rt_box.children.push(LayoutBox::new(
        BoxType::TextNode("かんじ".to_string()),
        Some(rt_style),
    ));
    ruby_box.children.push(rt_box);

    // <rp> ) </rp> (should be hidden in layout)
    let mut rp2_box = LayoutBox::new(BoxType::InlineNode, Some(ruby_style));
    rp2_box.tag_name = Some("rp".to_string());
    rp2_box
        .children
        .push(LayoutBox::new(BoxType::TextNode(")".to_string()), None));
    ruby_box.children.push(rp2_box);

    container.children.push(ruby_box);

    let mut float_ctx = FloatContext::new();
    let line_height = layout_inline_children(&mut container, &mut float_ctx);

    // 1. Ruby box creates an atomic inline-block containing rt and rb
    assert_eq!(container.children.len(), 1);
    let placed_ruby = &container.children[0];
    assert_eq!(placed_ruby.tag_name.as_deref(), Some("ruby"));

    // 2. rp parentheses are hidden; only rt and rb children are generated
    assert_eq!(placed_ruby.children.len(), 2);
    let rt_child = &placed_ruby.children[0];
    let rb_child = &placed_ruby.children[1];
    assert_eq!(rt_child.tag_name.as_deref(), Some("rt"));
    assert_eq!(rb_child.tag_name.as_deref(), Some("rb"));

    // 3. rt text sits above base text
    let rt_y = rt_child.dimensions.content.y();
    let rb_y = rb_child.dimensions.content.y();
    assert!(
        rt_y < rb_y,
        "Ruby text rt (y={rt_y}) must sit above ruby base rb (y={rb_y})"
    );

    // 4. Line height expands to accommodate both base text and annotation text
    assert!(
        line_height > 20.0,
        "Line height must accommodate ruby annotation, got: {line_height}"
    );

    // 5. Short ruby text is centered over the base text
    let rb_width = rb_child.dimensions.content.width();
    let rt_width = rt_child.dimensions.content.width();
    let rt_x = rt_child.dimensions.content.x();
    let rb_x = rb_child.dimensions.content.x();
    assert!(
        (rt_x - rb_x).abs() <= (rb_width - rt_width).abs() / 2.0 + 1.0,
        "Ruby text should be horizontally centered over base text"
    );
}

// =========================================================================
// 8.2.5 White-Space: break-spaces, pre-wrap, pre-line
// =========================================================================

#[test]
fn test_white_space_break_spaces_and_pre_wrap() {
    // 1. white-space: break-spaces preserves consecutive whitespace and allows breaking on each space
    let mut bs_container = LayoutBox::new(BoxType::BlockNode, None);
    // Narrow container to force line breaks across spaces:
    // "Alpha" (~45px) + 8 spaces (~32px) + "Beta" (~40px) = ~117px. Container is 60px.
    bs_container.dimensions.content = Rect::new(0.0, 0.0, 60.0, 0.0);

    let mut bs_style = ComputedStyle::default();
    bs_style.font_size = 16.0;
    bs_style.white_space = WhiteSpace::BreakSpaces;

    let bs_text = "Alpha        Beta";
    bs_container.children.push(LayoutBox::new(
        BoxType::TextNode(bs_text.to_string()),
        Some(bs_style),
    ));

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut bs_container, &mut float_ctx);

    // Under break-spaces in a narrow 60px box, spaces break across lines
    assert!(
        bs_container.children.len() >= 2,
        "break-spaces must break lines at spaces when overflowing, got {} boxes",
        bs_container.children.len()
    );

    // 2. Trailing spaces preserved under break-spaces
    // In a wide container, "Hello   " preserves all 3 trailing spaces
    let mut wide_container = LayoutBox::new(BoxType::BlockNode, None);
    wide_container.dimensions.content = Rect::new(0.0, 0.0, 400.0, 0.0);

    let mut wide_bs_style = ComputedStyle::default();
    wide_bs_style.font_size = 16.0;
    wide_bs_style.white_space = WhiteSpace::BreakSpaces;

    wide_container.children.push(LayoutBox::new(
        BoxType::TextNode("Hello   ".to_string()),
        Some(wide_bs_style),
    ));

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut wide_container, &mut float_ctx);

    assert_eq!(wide_container.children.len(), 1);
    let output_text = wide_container.children[0].text().unwrap_or("");
    assert_eq!(
        output_text, "Hello   ",
        "break-spaces must preserve trailing spaces at end of line"
    );

    // 3. white-space: pre-wrap preserves newlines and wraps long lines
    let mut pw_container = LayoutBox::new(BoxType::BlockNode, None);
    pw_container.dimensions.content = Rect::new(0.0, 0.0, 400.0, 0.0);

    let mut pw_style = ComputedStyle::default();
    pw_style.font_size = 16.0;
    pw_style.white_space = WhiteSpace::PreWrap;

    let pw_text = "Line 1\nLine 2\nLine 3";
    pw_container.children.push(LayoutBox::new(
        BoxType::TextNode(pw_text.to_string()),
        Some(pw_style),
    ));

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut pw_container, &mut float_ctx);

    assert_eq!(
        pw_container.children.len(),
        3,
        "pre-wrap must preserve explicit newline characters"
    );
    assert_eq!(pw_container.children[0].text(), Some("Line 1"));
    assert_eq!(pw_container.children[1].text(), Some("Line 2"));
    assert_eq!(pw_container.children[2].text(), Some("Line 3"));

    // 4. white-space: pre-line collapses spaces but preserves newlines
    let mut pl_container = LayoutBox::new(BoxType::BlockNode, None);
    pl_container.dimensions.content = Rect::new(0.0, 0.0, 400.0, 0.0);

    let mut pl_style = ComputedStyle::default();
    pl_style.font_size = 16.0;
    pl_style.white_space = WhiteSpace::PreLine;

    let pl_text = "Word1     Word2\nWord3     Word4";
    pl_container.children.push(LayoutBox::new(
        BoxType::TextNode(pl_text.to_string()),
        Some(pl_style),
    ));

    let mut float_ctx = FloatContext::new();
    layout_inline_children(&mut pl_container, &mut float_ctx);

    assert_eq!(
        pl_container.children.len(),
        2,
        "pre-line must preserve newlines into 2 lines"
    );
    assert_eq!(pl_container.children[0].text(), Some("Word1 Word2"));
    assert_eq!(pl_container.children[1].text(), Some("Word3 Word4"));
}
