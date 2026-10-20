use super::*;

#[test]
fn test_bidi_classification() {
    assert_eq!(bidi_class('A'), BidiClass::L);
    assert_eq!(bidi_class('1'), BidiClass::EN);
    assert_eq!(bidi_class('م'), BidiClass::AL);
    assert_eq!(bidi_class('ש'), BidiClass::R);
    assert_eq!(bidi_class('('), BidiClass::ON);
    assert_eq!(bidi_class(' '), BidiClass::WS);
}

#[test]
fn test_bidi_mirroring() {
    assert_eq!(mirror_char('('), ')');
    assert_eq!(mirror_char(')'), '(');
    assert_eq!(mirror_char('['), ']');
    assert_eq!(mirror_char('«'), '»');
    assert_eq!(mirror_char('⟨'), '⟩');
    assert_eq!(mirror_char('⌈'), '⌉');
}

#[test]
fn test_pure_ltr() {
    let text = "Hello World";
    assert_eq!(reorder_bidi_text(text, false), "Hello World");
}

#[test]
fn test_pure_rtl_reordering() {
    let text = "سلام";
    // In LTR rasterization order, Arabic letters are reversed visually
    let expected: String = text.chars().rev().collect();
    assert_eq!(reorder_bidi_text(text, true), expected);
}

#[test]
fn test_mixed_bidi_parentheses_mirroring() {
    let text = "(سلام)";
    let reordered = reorder_bidi_text(text, true);
    assert!(
        reordered.starts_with('(') && reordered.ends_with(')'),
        "Parentheses must mirror in RTL: got {}",
        reordered
    );
}

#[test]
fn test_mixed_bidi_numbers() {
    let text = "رقم 123";
    let reordered = reorder_bidi_text(text, true);
    assert!(
        reordered.contains("123"),
        "Numbers must preserve LTR order: got {}",
        reordered
    );
}

#[test]
fn test_directional_marks_classification() {
    assert_eq!(bidi_class('\u{200E}'), BidiClass::L, "LRM must be class L");
    assert_eq!(bidi_class('\u{200F}'), BidiClass::R, "RLM must be class R");
}

#[test]
fn test_bom_is_bn() {
    assert_eq!(
        bidi_class('\u{FEFF}'),
        BidiClass::BN,
        "BOM must be class BN"
    );
}

#[test]
fn test_nested_overrides() {
    let text = "\u{202E}AB\u{202D}CD\u{202C}EF\u{202C}";
    let reordered = reorder_bidi_text(text, false);
    assert!(!reordered.is_empty(), "Nested override must produce output");
}

#[test]
fn test_isolate_formatting() {
    let text = "Hello \u{2066}World\u{2069} end";
    let reordered = reorder_bidi_text(text, false);
    assert!(
        reordered.contains("Hello") && reordered.contains("World"),
        "LRI/PDI isolates must preserve content: got {}",
        reordered
    );
}

#[test]
fn test_base_direction_with_isolates() {
    let text = "\u{2067}Hello\u{2069}مرحبا";
    assert!(
        resolve_base_direction(text, false),
        "Base direction should be RTL (Arabic after isolate)"
    );
}

#[test]
fn test_additional_rtl_scripts() {
    assert_eq!(bidi_class('\u{0780}'), BidiClass::R);
    assert_eq!(bidi_class('\u{07C0}'), BidiClass::R);
    assert_eq!(bidi_class('\u{0710}'), BidiClass::AL);
}

#[test]
fn test_visual_runs_use_levels() {
    let text = "Hello سلام World";
    let runs = bidi_visual_runs(text, false);
    assert!(
        runs.len() >= 2,
        "Mixed text should produce multiple runs: got {:?}",
        runs
    );
    assert!(runs.iter().any(|r| r.is_rtl), "Must contain an RTL run");
}

#[test]
fn test_et_run_adjacent_to_en() {
    let text = "$$123";
    let reordered = reorder_bidi_text(text, false);
    assert_eq!(reordered, "$$123", "ET run before EN must stay LTR");
}

#[test]
fn test_math_bracket_mirroring() {
    assert_eq!(mirror_char('⟨'), '⟩');
    assert_eq!(mirror_char('⟩'), '⟨');
    assert_eq!(mirror_char('≤'), '≥');
    assert_eq!(mirror_char('≥'), '≤');
    assert_eq!(mirror_char('⊂'), '⊃');
    assert_eq!(mirror_char('⊃'), '⊂');
}

#[test]
fn test_embedding_codes_lre_rle() {
    let text = "\u{202A}Hello\u{202C}";
    let reordered = reorder_bidi_text(text, false);
    assert!(
        reordered.contains("Hello"),
        "LRE/PDF embedding must preserve content"
    );

    let text2 = "\u{202B}Hello\u{202C}";
    let reordered2 = reorder_bidi_text(text2, false);
    assert!(
        !reordered2.is_empty(),
        "RLE/PDF embedding must produce output"
    );
}

#[test]
fn test_fullwidth_digits_and_currency() {
    assert_eq!(bidi_class('\u{FF10}'), BidiClass::EN);
    assert_eq!(bidi_class('\u{FF19}'), BidiClass::EN);
    assert_eq!(bidi_class('¥'), BidiClass::ET);
    assert_eq!(bidi_class('€'), BidiClass::ET);
}

#[test]
fn test_whitespace_ranges() {
    assert_eq!(bidi_class('\u{2003}'), BidiClass::WS);
    assert_eq!(bidi_class('\u{2009}'), BidiClass::WS);
    assert_eq!(bidi_class('\u{3000}'), BidiClass::WS);
}

#[test]
fn test_bracket_pair_algorithm_n0() {
    let text = "سلام (hello) شكرا";
    let reordered = reorder_bidi_text(text, true);
    assert!(!reordered.is_empty());
    assert_eq!(bracket_type('('), Some(BracketType::Open(')')));
    assert_eq!(bracket_type(')'), Some(BracketType::Close('(')));
    assert_eq!(bracket_type('【'), Some(BracketType::Open('】')));
    assert_eq!(bracket_type('】'), Some(BracketType::Close('【')));
    assert_eq!(bracket_type('⟦'), Some(BracketType::Open('⟧')));
    assert_eq!(bracket_type('⟧'), Some(BracketType::Close('⟦')));
}

#[test]
fn test_visual_runs_matches_reordered_text() {
    let text = "Hello سلام World 123";
    let runs = bidi_visual_runs(text, false);
    let concatenated: String = runs.into_iter().map(|r| r.text).collect();
    let direct = reorder_bidi_text(text, false);
    assert_eq!(
        concatenated, direct,
        "Visual runs concatenated must match reorder_bidi_text output"
    );

    let rtl_text = "سلام 123 Hello شكرا";
    let rtl_runs = bidi_visual_runs(rtl_text, true);
    let rtl_concatenated: String = rtl_runs.into_iter().map(|r| r.text).collect();
    let rtl_direct = reorder_bidi_text(rtl_text, true);
    assert_eq!(
        rtl_concatenated, rtl_direct,
        "RTL visual runs concatenated must match reorder_bidi_text output"
    );
}

#[test]
fn test_extended_rtl_scripts() {
    assert_eq!(bidi_class('\u{1E900}'), BidiClass::R);
    assert_eq!(bidi_class('\u{10D00}'), BidiClass::R);
    assert_eq!(bidi_class('\u{10840}'), BidiClass::R);
    assert_eq!(bidi_class('\u{10900}'), BidiClass::R);
    assert_eq!(bidi_class('\u{1EE00}'), BidiClass::AL);
}

#[test]
fn test_extended_bracket_mirroring() {
    assert_eq!(mirror_char('⟦'), '⟧');
    assert_eq!(mirror_char('⟧'), '⟦');
    assert_eq!(mirror_char('≪'), '≫');
    assert_eq!(mirror_char('≫'), '≪');
    assert_eq!(mirror_char('≺'), '≻');
    assert_eq!(mirror_char('≻'), '≺');
}
