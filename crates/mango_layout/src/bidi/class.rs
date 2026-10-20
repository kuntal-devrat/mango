/// Unicode Bidirectional character classification types per UAX#9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiClass {
    // Strong types
    L,  // Left-to-Right
    R,  // Right-to-Left
    AL, // Right-to-Left Arabic
    // Weak types
    EN,  // European Number
    ES,  // European Separator
    ET,  // European Terminator
    AN,  // Arabic Number
    CS,  // Common Separator
    NSM, // Nonspacing Mark
    BN,  // Boundary Neutral
    // Neutral types
    B,  // Paragraph Separator
    S,  // Segment Separator
    WS, // Whitespace
    ON, // Other Neutral
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
        matches!(
            self,
            BidiClass::B | BidiClass::S | BidiClass::WS | BidiClass::ON
        )
    }

    /// Returns true for isolate initiator types (LRI, RLI, FSI).
    pub fn is_isolate_initiator(self) -> bool {
        matches!(self, BidiClass::LRI | BidiClass::RLI | BidiClass::FSI)
    }
}

/// Classifies a character into its UAX#9 directional property.
///
/// Covers the most commonly encountered scripts and formatting characters.
/// Characters not explicitly matched default to Left-to-Right (L), which is
/// correct for Latin, Greek, Cyrillic, CJK, Devanagari, Thai, and most other
/// LTR scripts.
pub fn bidi_class(ch: char) -> BidiClass {
    match ch {
        // Explicit directional marks — these are STRONG, not BN
        '\u{200E}' => BidiClass::L,   // LEFT-TO-RIGHT MARK
        '\u{200F}' => BidiClass::R,   // RIGHT-TO-LEFT MARK

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

        // Boundary neutrals & non-characters (BN)
        '\u{200B}' // ZERO WIDTH SPACE
        | '\u{200C}' // ZWNJ
        | '\u{200D}' // ZWJ
        | '\u{00AD}' // SOFT HYPHEN
        | '\u{FEFF}' // BOM / ZWNBSP
        | '\u{2060}' // WORD JOINER
        | '\u{2061}'..='\u{2064}' // invisible math operators
        => BidiClass::BN,

        // Paragraph and segment separators
        '\n' | '\r' | '\u{0085}' | '\u{2029}' => BidiClass::B,
        '\t' | '\u{000B}' | '\u{001F}' | '\u{001C}'..='\u{001E}' => BidiClass::S,

        // Whitespace (WS)
        ' '
        | '\u{000C}' // FORM FEED
        | '\u{00A0}' // NO-BREAK SPACE (also CS in some contexts, UAX#9 says WS)
        | '\u{1680}' // OGHAM SPACE
        | '\u{2000}'..='\u{200A}' // EN QUAD through HAIR SPACE (G3)
        | '\u{2028}' // LINE SEPARATOR
        | '\u{205F}' // MEDIUM MATHEMATICAL SPACE
        | '\u{3000}' // IDEOGRAPHIC SPACE
        => BidiClass::WS,

        // Arabic characters (AL)
        '\u{0600}'..='\u{0605}'
        | '\u{0608}'
        | '\u{060B}'
        | '\u{060D}'
        | '\u{061B}'..='\u{061F}'
        | '\u{0620}'..='\u{064A}' // Arabic letters: Hamza through Yeh (including Tatweel 0640 and Feh..Yeh 0641..064A)
        | '\u{066A}' // ARABIC PERCENT
        | '\u{066D}' // ARABIC FIVE POINTED STAR
        | '\u{066E}'..='\u{066F}'
        | '\u{0671}'..='\u{06D5}'
        | '\u{06E5}'..='\u{06E6}'
        | '\u{06EE}'..='\u{06EF}'
        | '\u{06FA}'..='\u{06FF}'
        | '\u{0750}'..='\u{077F}' // Arabic Supplement
        | '\u{0870}'..='\u{089F}' // Arabic Extended-B
        | '\u{08A0}'..='\u{08FF}' // Arabic Extended-A
        | '\u{FB50}'..='\u{FDFF}' // Arabic Presentation Forms-A
        | '\u{FE70}'..='\u{FEFC}'
        => BidiClass::AL,

        // Arabic Nonspacing Marks (NSM) — Tashkeel / diacritics
        '\u{064B}'..='\u{065F}'
        | '\u{0670}'
        | '\u{06D6}'..='\u{06DC}'
        | '\u{06DF}'..='\u{06E4}'
        | '\u{06E7}'..='\u{06E8}'
        | '\u{06EA}'..='\u{06ED}'
        => BidiClass::NSM,

        // Arabic-Indic Digits (AN)
        '\u{0660}'..='\u{0669}' | '\u{066B}' | '\u{066C}' => BidiClass::AN,
        // Extended Arabic-Indic Digits (AN)
        '\u{06F0}'..='\u{06F9}' => BidiClass::EN, // Eastern Arabic-Indic: these are EN per UAX#9

        // Hebrew characters (R)
        '\u{05D0}'..='\u{05EA}' // Hebrew letters
        | '\u{05EF}'..='\u{05F4}' // Hebrew Yod Triangle, punctuation
        | '\u{FB1D}'..='\u{FB4F}' // Hebrew presentation forms
        => BidiClass::R,

        // Hebrew Nonspacing Marks (NSM)
        '\u{0591}'..='\u{05BD}'
        | '\u{05BF}'
        | '\u{05C1}'..='\u{05C2}'
        | '\u{05C4}'..='\u{05C5}'
        | '\u{05C7}'
        => BidiClass::NSM,

        // Additional R scripts
        // Thaana (Maldivian)
        '\u{0780}'..='\u{07BF}' => BidiClass::R,
        // N'Ko
        '\u{07C0}'..='\u{07FF}' => BidiClass::R,
        // Samaritan
        '\u{0800}'..='\u{083F}' => BidiClass::R,
        // Mandaic
        '\u{0840}'..='\u{085F}' => BidiClass::R,
        // Imperial Aramaic
        '\u{10840}'..='\u{1085F}' => BidiClass::R,
        // Phoenician
        '\u{10900}'..='\u{1091F}' => BidiClass::R,
        // Kharoshthi
        '\u{10A00}'..='\u{10A5F}' => BidiClass::R,
        // Old South Arabian
        '\u{10A60}'..='\u{10A7F}' => BidiClass::R,
        // Hanifi Rohingya
        '\u{10D00}'..='\u{10D39}' => BidiClass::R,
        // Adlam
        '\u{1E900}'..='\u{1E95F}' => BidiClass::R,

        // Syriac (AL)
        '\u{0700}'..='\u{074F}' | '\u{0860}'..='\u{086F}' => BidiClass::AL,
        // Arabic Mathematical Alphabetic Symbols (AL)
        '\u{1EE00}'..='\u{1EEFF}' => BidiClass::AL,

        // European Numbers (EN)
        '0'..='9' => BidiClass::EN,
        '\u{2070}' | '\u{2074}'..='\u{2079}' // Superscript digits
        | '\u{2080}'..='\u{2089}' // Subscript digits
        | '\u{FF10}'..='\u{FF19}' // Fullwidth digits
        => BidiClass::EN,

        // European Separators (ES)
        '+' | '-'
        | '\u{FF0B}' | '\u{FF0D}' // Fullwidth plus/minus
        | '\u{2212}' // MINUS SIGN
        => BidiClass::ES,

        // European Terminators (ET) — currency symbols and related
        '$' | '%' | '¢' | '£' | '€' | '¥' | '°'
        | '\u{2030}' | '\u{2031}' // PER MILLE / PER TEN THOUSAND
        | '\u{20A0}'..='\u{20CF}' // Currency symbols block
        | '\u{FF04}' | '\u{FFE0}' | '\u{FFE1}' | '\u{FFE5}' | '\u{FFE6}' // Fullwidth currency
        | '#'
        => BidiClass::ET,

        // Common Separators (CS)
        ':' | ',' | '.' | '/'
        | '\u{FF0C}' | '\u{FF0E}' | '\u{FF1A}' // Fullwidth comma, period, colon
        | '\u{2044}' // FRACTION SLASH
        => BidiClass::CS,

        // Nonspacing Marks (General Unicode Diacritics) (NSM)
        '\u{0300}'..='\u{036F}' // Combining Diacritical Marks
        | '\u{0483}'..='\u{0489}' // Combining Cyrillic
        | '\u{0901}'..='\u{0903}' // Devanagari signs
        | '\u{093A}'..='\u{093C}'
        | '\u{093E}'..='\u{094F}'
        | '\u{0951}'..='\u{0957}'
        | '\u{0962}'..='\u{0963}'
        | '\u{0E31}' // Thai Mai Han Akat
        | '\u{0E34}'..='\u{0E3A}'
        | '\u{0E47}'..='\u{0E4E}'
        | '\u{1AB0}'..='\u{1AFF}' // Combining Diacritical Marks Extended
        | '\u{1DC0}'..='\u{1DFF}' // Combining Diacritical Marks Supplement
        | '\u{20D0}'..='\u{20FF}' // Combining Diacritical Marks for Symbols
        | '\u{FE20}'..='\u{FE2F}' // Combining Half Marks
        => BidiClass::NSM,

        // Other Neutrals (brackets, punctuation, symbols) (ON)
        '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | '«' | '»' | '‹' | '›' | '"' | '\''
        | '!' | '?' | ';' | '=' | '*' | '&' | '@' | '|' | '\\' | '^' | '~' | '`'
        | '‘' | '’' | '“' | '”'
        | '¡' | '¿' | '¦' | '§' | '©' | '®' | '†' | '‡' | '•' | '…'
        // CJK paired punctuation
        | '（' | '）' | '【' | '】' | '〔' | '〕' | '〈' | '〉' | '《' | '》' | '「' | '」'
        | '『' | '』'
        // Mathematical brackets
        | '⟨' | '⟩' | '⟪' | '⟫' | '⌈' | '⌉' | '⌊' | '⌋'
        => BidiClass::ON,

        // Default to Left-to-Right for Latin, Greek, Cyrillic, CJK, Devanagari, Thai, etc.
        _ => BidiClass::L,
    }
}
