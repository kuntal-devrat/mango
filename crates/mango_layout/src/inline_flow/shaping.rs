use mango_css::computed::ComputedStyle;

pub fn is_combining_mark(ch: char) -> bool {
    matches!(ch,
        '\u{0300}'..='\u{036F}' // Combining Diacritical Marks
        | '\u{1AB0}'..='\u{1AFF}' // Combining Diacritical Marks Extended
        | '\u{1DC0}'..='\u{1DFF}' // Combining Diacritical Marks Supplement
        | '\u{20D0}'..='\u{20FF}' // Combining Diacritical Marks for Symbols
        | '\u{FE20}'..='\u{FE2F}' // Combining Half Marks
        | '\u{0E31}' | '\u{0E34}'..='\u{0E3A}' | '\u{0E47}'..='\u{0E4E}' // Thai combining vowels and tones
        | '\u{064B}'..='\u{065F}' | '\u{0670}' // Arabic Tashkeel (Fatha, Damma, Kasra, Shadda, Sukun)
        | '\u{0591}'..='\u{05BD}' | '\u{05BF}' | '\u{05C1}' | '\u{05C2}' | '\u{05C4}' | '\u{05C5}' | '\u{05C7}' // Hebrew Niqqud / Cantillation
        | '\u{0901}'..='\u{0903}' | '\u{093C}' | '\u{093E}'..='\u{094C}' | '\u{094D}' | '\u{0951}'..='\u{0954}' // Devanagari vowel signs & virama
    )
}

/// Returns true if a character belongs to CJK writing systems (which break lines at character boundaries).
pub fn is_cjk_char(ch: char) -> bool {
    if is_combining_mark(ch) {
        return false;
    }
    matches!(ch,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
        | '\u{20000}'..='\u{2FA1F}' // CJK Extension B-I
        | '\u{F900}'..='\u{FAFF}' // CJK Compatibility
        | '\u{3040}'..='\u{309F}' // Hiragana
        | '\u{30A0}'..='\u{30FF}' // Katakana
        | '\u{31F0}'..='\u{31FF}' // Katakana Extensions
        | '\u{AC00}'..='\u{D7AF}' // Hangul Syllables
        | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' // Hangul Jamo
        | '\u{3100}'..='\u{312F}' // Bopomofo
    )
}

/// Returns true if a character is CJK closing punctuation (which cannot begin a line).
pub fn is_cjk_closing_punct(ch: char) -> bool {
    matches!(
        ch,
        '）' | '」'
            | '』'
            | '】'
            | '》'
            | '”'
            | '’'
            | '〕'
            | '〗'
            | '〙'
            | '。'
            | '，'
            | '、'
            | '；'
            | '：'
            | '！'
            | '？'
            | ')'
            | ']'
            | '}'
            | '>'
            | '!'
            | '?'
            | ','
            | '.'
            | ':'
            | ';'
    )
}

/// Returns true if a character is CJK opening punctuation (which cannot end a line).
pub fn is_cjk_opening_punct(ch: char) -> bool {
    matches!(
        ch,
        '（' | '「'
            | '『'
            | '【'
            | '《'
            | '“'
            | '‘'
            | '〔'
            | '〖'
            | '〘'
            | '('
            | '['
            | '{'
            | '<'
    )
}

/// Returns true if a character belongs to an RTL script (Arabic or Hebrew).
pub fn is_rtl_char(ch: char) -> bool {
    matches!(ch,
        '\u{0590}'..='\u{05FF}' // Hebrew
        | '\u{0600}'..='\u{06FF}' // Arabic
        | '\u{0750}'..='\u{077F}' // Arabic Supplement
        | '\u{08A0}'..='\u{08FF}' // Arabic Extended-A
        | '\u{FB50}'..='\u{FDFF}' // Arabic Presentation Forms-A
        | '\u{FE70}'..='\u{FEFF}' // Arabic Presentation Forms-B
    )
}

/// Shapes complex scripts (Arabic, Devanagari, Thai) and visually orders RTL text with paragraph direction using UAX#9.
pub fn shape_and_bidi_text_with_dir(text: &str, is_rtl_dir: bool) -> String {
    let shaped = crate::shaping::shape_complex_script(text);
    if !shaped.chars().any(is_rtl_char) && !is_rtl_dir {
        return shaped;
    }
    crate::bidi::reorder_bidi_text(&shaped, is_rtl_dir)
}

/// Shapes complex scripts and visually orders RTL text for LTR rasterization.
pub fn shape_and_bidi_text(text: &str) -> String {
    shape_and_bidi_text_with_dir(text, false)
}

/// Returns true if a font weight should be rendered with bold glyphs.
/// Weights >= 600 are bold; medium (500) is bold only for large display headings (>= 32px).
pub fn is_bold_weight(weight: mango_css::values::FontWeight, font_size: f32) -> bool {
    match weight {
        mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
        mango_css::values::FontWeight::Numeric(w) => w >= 600 || (w >= 420 && font_size >= 32.0),
        _ => false,
    }
}

/// Resolves the render weight for a computed style (CSS 400/700 → Regular/Bold).
pub(crate) fn font_weight_for(style: &ComputedStyle) -> mango_render::FontWeight {
    if is_bold_weight(style.font_weight, style.font_size) {
        mango_render::FontWeight::Bold
    } else {
        mango_render::FontWeight::Regular
    }
}

/// Resolves the render font family for a computed style.
pub(crate) fn font_family_for(style: &ComputedStyle) -> mango_render::FontFamily {
    mango_render::FontFamily::from_css_name(&style.font_family)
}

/// The used line height of a run: the declared `line-height`, or the font's own
/// `ascent + descent + line-gap` for `normal` — which is how Chromium computes it,
/// rather than a fixed multiple of the font size (CSS 2.1 §10.8.1).
pub(crate) fn used_line_height(
    style: &ComputedStyle,
    family: mango_render::FontFamily,
    weight: mango_render::FontWeight,
) -> f32 {
    if let Some(explicit) = style.line_height {
        return explicit.max(0.0);
    }
    let (ascent, descent, gap) =
        mango_render::font::font_manager().font_metrics(family, weight, style.font_size);
    (ascent + descent + gap).max(1.0)
}
