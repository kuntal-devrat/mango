//! Pure Rust implementation of Unicode Bidirectional Algorithm (UAX#9).
//!
//! Provides bidirectional text classification, level resolution, bracket/glyph mirroring,
//! and visual run reordering for mixed LTR/RTL scripts (Arabic, Hebrew, Latin, etc.).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiClass {
    // Strong types
    L,   // Left-to-Right
    R,   // Right-to-Left
    AL,  // Right-to-Left Arabic
    // Weak types
    EN,  // European Number
    ES,  // European Separator
    ET,  // European Terminator
    AN,  // Arabic Number
    CS,  // Common Separator
    NSM, // Nonspacing Mark
    BN,  // Boundary Neutral
    // Neutral types
    B,   // Paragraph Separator
    S,   // Segment Separator
    WS,  // Whitespace
    ON,  // Other Neutral
    // Explicit formatting codes
    LRE,
    RLE,
    LRO,
    RLO,
    PDF,
    LRI,
    RLI,
    FSI,
    PDI,
}

impl BidiClass {
    pub fn is_strong(self) -> bool {
        matches!(self, BidiClass::L | BidiClass::R | BidiClass::AL)
    }

    pub fn is_neutral(self) -> bool {
        matches!(self, BidiClass::B | BidiClass::S | BidiClass::WS | BidiClass::ON)
    }
}

/// Classifies a character into its UAX#9 directional property.
pub fn bidi_class(ch: char) -> BidiClass {
    match ch {
        // Explicit directional formatting characters
        '\u{202A}' => BidiClass::LRE,
        '\u{202B}' => BidiClass::RLE,
        '\u{202C}' => BidiClass::PDF,
        '\u{202D}' => BidiClass::LRO,
        '\u{202E}' => BidiClass::RLO,
        '\u{2066}' => BidiClass::LRI,
        '\u{2067}' => BidiClass::RLI,
        '\u{2068}' => BidiClass::FSI,
        '\u{2069}' => BidiClass::PDI,

        // Boundary neutrals & non-characters
        '\u{200C}' | '\u{200D}' | '\u{200E}' | '\u{200F}' | '\u{00AD}' | '\u{FEFF}' => {
            BidiClass::BN
        }

        // Paragraph and segment separators
        '\n' | '\r' | '\u{2029}' => BidiClass::B,
        '\t' | '\u{001F}' => BidiClass::S,
        ' ' | '\u{3000}' => BidiClass::WS,

        // Arabic characters (AL)
        '\u{0600}'..='\u{0605}'
        | '\u{0608}'
        | '\u{060B}'
        | '\u{0620}'..='\u{065F}'
        | '\u{066E}'..='\u{06D5}'
        | '\u{06E5}'..='\u{06EF}'
        | '\u{06FA}'..='\u{06FF}'
        | '\u{0750}'..='\u{077F}'
        | '\u{08A0}'..='\u{08FF}'
        | '\u{FB50}'..='\u{FDFF}'
        | '\u{FE70}'..='\u{FEFF}' => {
            // Check non-spacing Tashkeel / diacritic marks
            if matches!(ch, '\u{064B}'..='\u{065F}' | '\u{0670}' | '\u{06D6}'..='\u{06DC}' | '\u{06DF}'..='\u{06E4}' | '\u{06E7}' | '\u{06E8}' | '\u{06EA}'..='\u{06ED}') {
                BidiClass::NSM
            } else {
                BidiClass::AL
            }
        }

        // Arabic-Indic Digits (AN)
        '\u{0660}'..='\u{0669}' | '\u{066B}' | '\u{066C}' => BidiClass::AN,

        // Hebrew characters (R)
        '\u{0590}'..='\u{05FF}' | '\u{FB1D}'..='\u{FB4F}' => {
            if matches!(ch, '\u{0591}'..='\u{05BD}' | '\u{05BF}' | '\u{05C1}' | '\u{05C2}' | '\u{05C4}' | '\u{05C5}' | '\u{05C7}') {
                BidiClass::NSM
            } else {
                BidiClass::R
            }
        }

        // European Numbers (EN)
        '0'..='9' => BidiClass::EN,

        // European Separators (ES)
        '+' | '-' => BidiClass::ES,

        // European Terminators (ET)
        '$' | '%' | '¢' | '£' | '€' | '°' | '\u{2030}' | '\u{2031}' => BidiClass::ET,

        // Common Separators (CS)
        ':' | ',' | '.' | '/' | '\u{00A0}' => BidiClass::CS,

        // Nonspacing Marks (General Unicode Diacritics)
        '\u{0300}'..='\u{036F}'
        | '\u{1AB0}'..='\u{1AFF}'
        | '\u{1DC0}'..='\u{1DFF}'
        | '\u{20D0}'..='\u{20FF}'
        | '\u{FE20}'..='\u{FE2F}'
        | '\u{0901}'..='\u{0903}' | '\u{093C}' | '\u{094D}' | '\u{0951}'..='\u{0954}'
        | '\u{0E31}' | '\u{0E34}'..='\u{0E3A}' | '\u{0E47}'..='\u{0E4E}' => BidiClass::NSM,

        // Other Neutrals (brackets, punctuation, symbols)
        '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
        | '«' | '»' | '‹' | '›' | '“' | '”' | '‘' | '’'
        | '!' | '?' | ';' | '=' | '*' | '&' | '@' | '#' | '|' | '\\' | '^' | '~' | '`'
        | '（' | '）' | '【' | '】' | '〔' | '〕' | '〈' | '〉' | '《' | '》'
        | '「' | '」' | '『' | '』' => BidiClass::ON,

        // Default to Left-to-Right for Latin, Greek, Cyrillic, CJK, Devanagari, Thai, etc.
        _ => BidiClass::L,
    }
}

/// Returns the mirrored glyph for paired characters according to UAX#9 rule L4.
pub fn mirror_char(ch: char) -> char {
    match ch {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        '‹' => '›',
        '›' => '‹',
        '“' => '”',
        '”' => '“',
        '‘' => '’',
        '’' => '‘',
        '（' => '）',
        '）' => '（',
        '【' => '】',
        '】' => '【',
        '〔' => '〕',
        '〕' => '〔',
        '〈' => '〉',
        '〉' => '〈',
        '《' => '》',
        '》' => '《',
        '「' => '」',
        '」' => '「',
        '『' => '』',
        '』' => '『',
        _ => ch,
    }
}

/// Represents a visual directional run after BiDi level resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BidiRun {
    pub text: String,
    pub level: u8,
    pub is_rtl: bool,
}

/// Resolves paragraph base direction (UAX#9 Rules P2–P3).
pub fn resolve_base_direction(text: &str, default_rtl: bool) -> bool {
    for ch in text.chars() {
        match bidi_class(ch) {
            BidiClass::R | BidiClass::AL => return true,
            BidiClass::L => return false,
            _ => {}
        }
    }
    default_rtl
}

/// Executes the Unicode Bidirectional Algorithm (UAX#9) on a paragraph of text.
///
/// Returns visually ordered characters ready for left-to-right rasterization.
pub fn reorder_bidi_text(text: &str, is_rtl_base: bool) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }

    let n = chars.len();
    let base_level: u8 = if is_rtl_base { 1 } else { 0 };

    // Initial classes
    let mut types: Vec<BidiClass> = chars.iter().copied().map(bidi_class).collect();
    let mut levels: Vec<u8> = vec![base_level; n];

    // Explicit overrides (LRO, RLO, PDF)
    let mut current_override: Option<BidiClass> = None;
    for i in 0..n {
        match types[i] {
            BidiClass::LRO => {
                current_override = Some(BidiClass::L);
                types[i] = BidiClass::BN;
            }
            BidiClass::RLO => {
                current_override = Some(BidiClass::R);
                types[i] = BidiClass::BN;
            }
            BidiClass::PDF => {
                current_override = None;
                types[i] = BidiClass::BN;
            }
            _ => {
                if let Some(ovr) = current_override {
                    types[i] = ovr;
                }
            }
        }
    }

    // --- Weak Types Resolution (W1–W7) ---
    // W1: NSM takes type of preceding character
    let mut prev_type = if is_rtl_base { BidiClass::R } else { BidiClass::L };
    for i in 0..n {
        if types[i] == BidiClass::NSM {
            types[i] = prev_type;
        } else if types[i] != BidiClass::BN {
            prev_type = types[i];
        }
    }

    // W2: European Numbers (EN) after Arabic Letter (AL) become Arabic Numbers (AN)
    let mut last_strong = if is_rtl_base { BidiClass::R } else { BidiClass::L };
    for i in 0..n {
        if types[i].is_strong() {
            last_strong = types[i];
        } else if types[i] == BidiClass::EN && last_strong == BidiClass::AL {
            types[i] = BidiClass::AN;
        }
    }

    // W3: AL becomes R
    for t in &mut types {
        if *t == BidiClass::AL {
            *t = BidiClass::R;
        }
    }

    // W4: Single ES between ENs becomes EN; single CS between ENs becomes EN; CS between ANs becomes AN
    for i in 1..n.saturating_sub(1) {
        if types[i] == BidiClass::ES && types[i - 1] == BidiClass::EN && types[i + 1] == BidiClass::EN {
            types[i] = BidiClass::EN;
        } else if types[i] == BidiClass::CS {
            if types[i - 1] == BidiClass::EN && types[i + 1] == BidiClass::EN {
                types[i] = BidiClass::EN;
            } else if types[i - 1] == BidiClass::AN && types[i + 1] == BidiClass::AN {
                types[i] = BidiClass::AN;
            }
        }
    }

    // W5: ET adjacent to EN becomes EN
    for i in 0..n {
        if types[i] == BidiClass::ET {
            // Look forward for EN
            let mut has_en = false;
            let mut j = i;
            while j < n && types[j] == BidiClass::ET {
                j += 1;
            }
            if j < n && types[j] == BidiClass::EN {
                has_en = true;
            } else if i > 0 && types[i - 1] == BidiClass::EN {
                has_en = true;
            }
            if has_en {
                types[i] = BidiClass::EN;
            }
        }
    }

    // W6: Remaining ES, ET, CS become ON
    for t in &mut types {
        if matches!(*t, BidiClass::ES | BidiClass::ET | BidiClass::CS) {
            *t = BidiClass::ON;
        }
    }

    // W7: EN preceded by L becomes L
    let mut last_strong_l = false;
    for t in &mut types {
        if *t == BidiClass::L {
            last_strong_l = true;
        } else if *t == BidiClass::R {
            last_strong_l = false;
        } else if *t == BidiClass::EN && last_strong_l {
            *t = BidiClass::L;
        }
    }

    // --- Neutral Types Resolution (N1–N2) ---
    // N1: Neutrals bracketed by same direction type become that type
    let mut i = 0;
    while i < n {
        if types[i].is_neutral() {
            let start = i;
            while i < n && types[i].is_neutral() {
                i += 1;
            }
            let end = i;
            let lead_type = if start > 0 { types[start - 1] } else { if is_rtl_base { BidiClass::R } else { BidiClass::L } };
            let trail_type = if end < n { types[end] } else { if is_rtl_base { BidiClass::R } else { BidiClass::L } };

            let resolved = match (lead_type, trail_type) {
                (BidiClass::L, BidiClass::L) => BidiClass::L,
                (BidiClass::R, BidiClass::R)
                | (BidiClass::R, BidiClass::AN)
                | (BidiClass::AN, BidiClass::R)
                | (BidiClass::AN, BidiClass::AN) => BidiClass::R,
                _ => {
                    // N2: Remaining neutrals take base embedding direction
                    if is_rtl_base { BidiClass::R } else { BidiClass::L }
                }
            };
            for k in start..end {
                types[k] = resolved;
            }
        } else {
            i += 1;
        }
    }

    // --- Implicit Levels (I1–I2) ---
    for i in 0..n {
        let t = types[i];
        if base_level % 2 == 0 {
            // LTR base
            if t == BidiClass::R {
                levels[i] = base_level + 1;
            } else if t == BidiClass::AN || t == BidiClass::EN {
                levels[i] = base_level + 2;
            } else {
                levels[i] = base_level;
            }
        } else {
            // RTL base (odd)
            // Rule I2: For characters with an odd embedding level, L, EN, or AN have level increased by 1
            if t == BidiClass::L || t == BidiClass::EN || t == BidiClass::AN {
                levels[i] = base_level + 1;
            } else {
                levels[i] = base_level;
            }
        }
    }

    // --- L1: Reset trailing whitespace to base level ---
    let mut trail = n;
    while trail > 0 && bidi_class(chars[trail - 1]) == BidiClass::WS {
        trail -= 1;
        levels[trail] = base_level;
    }

    // --- L4: Mirroring in odd levels ---
    let mut out_chars = chars.clone();
    for i in 0..n {
        if levels[i] % 2 == 1 {
            out_chars[i] = mirror_char(out_chars[i]);
        }
    }

    // --- L2: Reversing Resolved Levels ---
    let max_level = levels.iter().copied().fold(0u8, u8::max);
    let min_odd_level = if is_rtl_base { 1 } else { levels.iter().copied().filter(|&l| l % 2 == 1).fold(255u8, u8::min) };

    if min_odd_level <= max_level {
        let mut lvl = max_level;
        loop {
            let mut s = 0;
            while s < n {
                if levels[s] >= lvl {
                    let mut e = s;
                    while e < n && levels[e] >= lvl {
                        e += 1;
                    }
                    out_chars[s..e].reverse();
                    levels[s..e].reverse();
                    s = e;
                } else {
                    s += 1;
                }
            }
            if lvl <= min_odd_level || lvl == 0 {
                break;
            }
            lvl -= 1;
        }
    }

    // Filter out boundary neutral formatting codes
    out_chars.into_iter().filter(|&c| !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')).collect()
}

/// Splits text into visual directional runs according to UAX#9 levels.
pub fn bidi_visual_runs(text: &str, is_rtl_base: bool) -> Vec<BidiRun> {
    let reordered = reorder_bidi_text(text, is_rtl_base);
    let mut runs = Vec::new();
    let mut current = String::new();
    let mut cur_is_rtl: Option<bool> = None;

    for ch in reordered.chars() {
        let rtl = matches!(bidi_class(ch), BidiClass::R | BidiClass::AL | BidiClass::AN);
        if let Some(c_rtl) = cur_is_rtl {
            if c_rtl != rtl && !ch.is_whitespace() {
                let text_seg = std::mem::take(&mut current);
                runs.push(BidiRun {
                    text: text_seg,
                    level: if c_rtl { 1 } else { 0 },
                    is_rtl: c_rtl,
                });
                cur_is_rtl = Some(rtl);
            }
        } else if !ch.is_whitespace() {
            cur_is_rtl = Some(rtl);
        }
        current.push(ch);
    }

    if !current.is_empty() {
        let is_rtl = cur_is_rtl.unwrap_or(is_rtl_base);
        runs.push(BidiRun {
            text: current,
            level: if is_rtl { 1 } else { 0 },
            is_rtl,
        });
    }

    runs
}

#[cfg(test)]
mod tests {
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
        // Parentheses mirrored: '(' at start becomes ')' when reversed in RTL, and then L4 mirrors it back to '(' at visually left end
        assert!(reordered.starts_with('(') && reordered.ends_with(')'), "Parentheses must mirror in RTL: got {}", reordered);
    }

    #[test]
    fn test_mixed_bidi_numbers() {
        let text = "رقم 123";
        let reordered = reorder_bidi_text(text, true);
        // European numbers remain in visual left-to-right order (123)
        assert!(reordered.contains("123"), "Numbers must preserve LTR order: got {}", reordered);
    }
}
