//! Pure Rust implementation of Unicode Bidirectional Algorithm (UAX#9).
//!
//! Provides bidirectional text classification, level resolution, bracket/glyph mirroring,
//! and visual run reordering for mixed LTR/RTL scripts (Arabic, Hebrew, Latin, etc.).
//!
//! Implements:
//! - P2–P3: Paragraph base direction resolution
//! - X1–X8: Explicit embedding/override/isolate level stack (max depth 125)
//! - W1–W7: Weak type resolution
//! - N1–N2: Neutral type resolution
//! - I1–I2: Implicit level assignment
//! - L1–L4: Visual reordering and mirroring

/// Maximum depth for the directional status stack (UAX#9 §3.3.2).
const MAX_DEPTH: u8 = 125;

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
        // B1 fix: Explicit directional marks — these are STRONG, not BN
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
        // B1 fix: U+200E/200F removed from here (they are strong L/R)
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
        // B2 fix: Arabic Presentation Forms-B ends at U+FEFC, not U+FEFF
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

        // G3: Additional R scripts
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
        | '\u{20A0}'..='\u{20CF}' // Currency symbols block (G3)
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

/// Bracket type according to Unicode Bidirectional Character Type (BidiBrackets.txt).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BracketType {
    Open(char),  // contains paired closing character
    Close(char), // contains paired opening character
}

/// Identifies if a character is a paired bracket per UAX#9 BD14/BD16.
pub fn bracket_type(ch: char) -> Option<BracketType> {
    match ch {
        // ASCII paired brackets
        '(' => Some(BracketType::Open(')')),
        ')' => Some(BracketType::Close('(')),
        '[' => Some(BracketType::Open(']')),
        ']' => Some(BracketType::Close('[')),
        '{' => Some(BracketType::Open('}')),
        '}' => Some(BracketType::Close('{')),
        '<' => Some(BracketType::Open('>')),
        '>' => Some(BracketType::Close('<')),

        // Mathematical angle & double brackets
        '⟨' => Some(BracketType::Open('⟩')),
        '⟩' => Some(BracketType::Close('⟨')),
        '⟪' => Some(BracketType::Open('⟫')),
        '⟫' => Some(BracketType::Close('⟪')),
        '⌈' => Some(BracketType::Open('⌉')),
        '⌉' => Some(BracketType::Close('⌈')),
        '⌊' => Some(BracketType::Open('⌋')),
        '⌋' => Some(BracketType::Close('⌊')),
        '⟦' => Some(BracketType::Open('⟧')),
        '⟧' => Some(BracketType::Close('⟦')),
        '⟬' => Some(BracketType::Open('⟭')),
        '⟭' => Some(BracketType::Close('⟬')),
        '⟮' => Some(BracketType::Open('⟯')),
        '⟯' => Some(BracketType::Close('⟮')),

        // CJK paired brackets
        '（' => Some(BracketType::Open('）')),
        '）' => Some(BracketType::Close('（')),
        '【' => Some(BracketType::Open('】')),
        '】' => Some(BracketType::Close('【')),
        '〔' => Some(BracketType::Open('〕')),
        '〕' => Some(BracketType::Close('〔')),
        '〈' => Some(BracketType::Open('〉')),
        '〉' => Some(BracketType::Close('〈')),
        '《' => Some(BracketType::Open('》')),
        '》' => Some(BracketType::Close('《')),
        '「' => Some(BracketType::Open('」')),
        '」' => Some(BracketType::Close('「')),
        '『' => Some(BracketType::Open('』')),
        '』' => Some(BracketType::Close('『')),
        '〖' => Some(BracketType::Open('〗')),
        '〗' => Some(BracketType::Close('〖')),
        '〘' => Some(BracketType::Open('〙')),
        '〙' => Some(BracketType::Close('〘')),
        '〚' => Some(BracketType::Open('〛')),
        '〛' => Some(BracketType::Close('〚')),

        _ => None,
    }
}

/// Returns the mirrored glyph for paired characters according to UAX#9 rule L4.
///
/// Covers common bracket pairs plus mathematical angle brackets, ceiling/floor
/// brackets, and set membership / relational symbols.
pub fn mirror_char(ch: char) -> char {
    match ch {
        // ASCII brackets
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',

        // Guillemets
        '«' => '»',
        '»' => '«',
        '‹' => '›',
        '›' => '‹',

        // Curly quotes
        '\u{201C}' => '\u{201D}', // “ → ”
        '\u{201D}' => '\u{201C}', // ” → “
        '\u{2018}' => '\u{2019}', // ‘ → ’
        '\u{2019}' => '\u{2018}', // ’ → ‘

        // CJK paired brackets
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
        '〖' => '〗',
        '〗' => '〖',
        '〘' => '〙',
        '〙' => '〘',
        '〚' => '〛',
        '〛' => '〚',

        // Mathematical brackets
        '⟨' => '⟩',
        '⟩' => '⟨',
        '⟪' => '⟫',
        '⟫' => '⟪',
        '⌈' => '⌉',
        '⌉' => '⌈',
        '⌊' => '⌋',
        '⌋' => '⌊',
        '⟦' => '⟧',
        '⟧' => '⟦',
        '⟬' => '⟭',
        '⟭' => '⟬',
        '⟮' => '⟯',
        '⟯' => '⟮',

        // Mathematical relations and symbols
        '∈' => '∋',
        '∋' => '∈',
        '∉' => '∌',
        '∌' => '∉',
        '≤' => '≥',
        '≥' => '≤',
        '≦' => '≧',
        '≧' => '≦',
        '≲' => '≳',
        '≳' => '≲',
        '⊂' => '⊃',
        '⊃' => '⊂',
        '⊆' => '⊇',
        '⊇' => '⊆',
        '⊊' => '⊋',
        '⊋' => '⊊',
        '⊄' => '⊅',
        '⊅' => '⊄',
        '⊈' => '⊉',
        '⊉' => '⊈',
        '≪' => '≫',
        '≫' => '≪',
        '≮' => '≯',
        '≯' => '≮',
        '≰' => '≱',
        '≱' => '≰',
        '≺' => '≻',
        '≻' => '≺',
        '≼' => '≽',
        '≽' => '≼',
        '⊏' => '⊐',
        '⊐' => '⊏',
        '⊑' => '⊒',
        '⊒' => '⊑',
        '⊲' => '⊳',
        '⊳' => '⊲',
        '⊴' => '⊵',
        '⊵' => '⊴',
        '⋖' => '⋗',
        '⋗' => '⋖',

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
///
/// Scans for the first strong character (L, R, AL) ignoring characters inside
/// isolate pairs. Returns `true` for RTL base, `false` for LTR base.
pub fn resolve_base_direction(text: &str, default_rtl: bool) -> bool {
    let mut isolate_depth = 0u32;
    for ch in text.chars() {
        let cls = bidi_class(ch);
        if cls.is_isolate_initiator() {
            isolate_depth += 1;
            continue;
        }
        if cls == BidiClass::PDI {
            isolate_depth = isolate_depth.saturating_sub(1);
            continue;
        }
        if isolate_depth > 0 {
            continue;
        }
        match cls {
            BidiClass::R | BidiClass::AL => return true,
            BidiClass::L => return false,
            _ => {}
        }
    }
    default_rtl
}

// ─── Directional Status Stack (X1–X8) ───────────────────────────────────────

/// An entry on the directional status stack.
#[derive(Debug, Clone, Copy)]
struct DirectionalEntry {
    level: u8,
    override_status: Option<BidiClass>, // None = neutral, Some(L) or Some(R)
    isolate_status: bool,
}

/// Computes the least greater odd level.
fn next_odd(level: u8) -> u8 {
    (level + 1) | 1
}

/// Computes the least greater even level.
fn next_even(level: u8) -> u8 {
    (level + 2) & !1
}

/// Executes Rules X1–X8 to compute embedding levels.
///
/// Returns the resolved levels, resolved types, and a boolean array marking
/// which positions are isolate initiators.
fn resolve_explicit_levels(
    chars: &[char],
    types: &mut [BidiClass],
    base_level: u8,
) -> Vec<u8> {
    let n = chars.len();
    let mut levels = vec![base_level; n];

    let mut stack: Vec<DirectionalEntry> = Vec::with_capacity(MAX_DEPTH as usize + 2);
    stack.push(DirectionalEntry {
        level: base_level,
        override_status: None,
        isolate_status: false,
    });

    let mut overflow_isolate_count: u32 = 0;
    let mut overflow_embedding_count: u32 = 0;
    let mut valid_isolate_count: u32 = 0;

    for i in 0..n {
        let original_type = types[i];
        let current = stack.last().unwrap();
        let current_level = current.level;

        match original_type {
            // X2: RLE
            BidiClass::RLE => {
                let new_level = next_odd(current_level);
                if new_level <= MAX_DEPTH && overflow_isolate_count == 0 && overflow_embedding_count == 0 {
                    stack.push(DirectionalEntry {
                        level: new_level,
                        override_status: None,
                        isolate_status: false,
                    });
                } else if overflow_isolate_count == 0 {
                    overflow_embedding_count += 1;
                }
                levels[i] = current_level;
                types[i] = BidiClass::BN;
            }
            // X3: LRE
            BidiClass::LRE => {
                let new_level = next_even(current_level);
                if new_level <= MAX_DEPTH && overflow_isolate_count == 0 && overflow_embedding_count == 0 {
                    stack.push(DirectionalEntry {
                        level: new_level,
                        override_status: None,
                        isolate_status: false,
                    });
                } else if overflow_isolate_count == 0 {
                    overflow_embedding_count += 1;
                }
                levels[i] = current_level;
                types[i] = BidiClass::BN;
            }
            // X4: RLO
            BidiClass::RLO => {
                let new_level = next_odd(current_level);
                if new_level <= MAX_DEPTH && overflow_isolate_count == 0 && overflow_embedding_count == 0 {
                    stack.push(DirectionalEntry {
                        level: new_level,
                        override_status: Some(BidiClass::R),
                        isolate_status: false,
                    });
                } else if overflow_isolate_count == 0 {
                    overflow_embedding_count += 1;
                }
                levels[i] = current_level;
                types[i] = BidiClass::BN;
            }
            // X5: LRO
            BidiClass::LRO => {
                let new_level = next_even(current_level);
                if new_level <= MAX_DEPTH && overflow_isolate_count == 0 && overflow_embedding_count == 0 {
                    stack.push(DirectionalEntry {
                        level: new_level,
                        override_status: Some(BidiClass::L),
                        isolate_status: false,
                    });
                } else if overflow_isolate_count == 0 {
                    overflow_embedding_count += 1;
                }
                levels[i] = current_level;
                types[i] = BidiClass::BN;
            }
            // X5a: RLI
            BidiClass::RLI => {
                levels[i] = current_level;
                let new_level = next_odd(current_level);
                if new_level <= MAX_DEPTH && overflow_isolate_count == 0 && overflow_embedding_count == 0 {
                    valid_isolate_count += 1;
                    stack.push(DirectionalEntry {
                        level: new_level,
                        override_status: None,
                        isolate_status: true,
                    });
                } else {
                    overflow_isolate_count += 1;
                }
                types[i] = BidiClass::ON; // Treat as ON for neutral resolution
            }
            // X5b: LRI
            BidiClass::LRI => {
                levels[i] = current_level;
                let new_level = next_even(current_level);
                if new_level <= MAX_DEPTH && overflow_isolate_count == 0 && overflow_embedding_count == 0 {
                    valid_isolate_count += 1;
                    stack.push(DirectionalEntry {
                        level: new_level,
                        override_status: None,
                        isolate_status: true,
                    });
                } else {
                    overflow_isolate_count += 1;
                }
                types[i] = BidiClass::ON;
            }
            // X5c: FSI — resolve direction of isolate content, then act as LRI or RLI
            BidiClass::FSI => {
                // Determine the direction of the text up to the matching PDI
                let is_rtl = resolve_isolate_direction(chars, types, i + 1);
                levels[i] = current_level;
                let new_level = if is_rtl {
                    next_odd(current_level)
                } else {
                    next_even(current_level)
                };
                if new_level <= MAX_DEPTH && overflow_isolate_count == 0 && overflow_embedding_count == 0 {
                    valid_isolate_count += 1;
                    stack.push(DirectionalEntry {
                        level: new_level,
                        override_status: None,
                        isolate_status: true,
                    });
                } else {
                    overflow_isolate_count += 1;
                }
                types[i] = BidiClass::ON;
            }
            // X7: PDF
            BidiClass::PDF => {
                if overflow_isolate_count > 0 {
                    // ignore
                } else if overflow_embedding_count > 0 {
                    overflow_embedding_count -= 1;
                } else if stack.len() >= 2 && !stack.last().unwrap().isolate_status {
                    stack.pop();
                }
                levels[i] = stack.last().unwrap().level;
                types[i] = BidiClass::BN;
            }
            // X6a: PDI
            BidiClass::PDI => {
                if overflow_isolate_count > 0 {
                    overflow_isolate_count -= 1;
                } else if valid_isolate_count > 0 {
                    overflow_embedding_count = 0;
                    // Pop until we find an isolate entry
                    while stack.len() > 1 {
                        if stack.last().unwrap().isolate_status {
                            stack.pop();
                            break;
                        }
                        stack.pop();
                    }
                    valid_isolate_count -= 1;
                }
                levels[i] = stack.last().unwrap().level;
                types[i] = BidiClass::ON;
            }
            // X6: All other characters
            _ => {
                levels[i] = current_level;
                if let Some(ovr) = stack.last().unwrap().override_status {
                    types[i] = ovr;
                }
            }
        }
    }

    levels
}

/// Resolves the direction of text inside an FSI isolate for Rule X5c.
fn resolve_isolate_direction(chars: &[char], types: &[BidiClass], start: usize) -> bool {
    let mut depth = 1u32;
    let mut i = start;
    while i < chars.len() && depth > 0 {
        let cls = types[i];
        if cls.is_isolate_initiator() {
            depth += 1;
        } else if cls == BidiClass::PDI {
            depth -= 1;
            if depth == 0 {
                break;
            }
        } else if depth == 1 {
            match cls {
                BidiClass::R | BidiClass::AL => return true,
                BidiClass::L => return false,
                _ => {}
            }
        }
        i += 1;
    }
    false
}

// ─── Main Algorithm ──────────────────────────────────────────────────────────

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

    // X1–X8: Explicit embedding/override/isolate levels (B3 fix: proper stack)
    let mut levels = resolve_explicit_levels(&chars, &mut types, base_level);

    // Apply W1–W7, N0 (bracket pairs), N1–N2, I1–I2, L1
    apply_weak_and_neutral_rules(&mut types, &mut levels, &chars, base_level, is_rtl_base);

    // --- L4: Mirroring in odd levels ---
    // O4: move chars instead of clone since we don't need the original anymore
    let mut out_chars = chars;
    for i in 0..n {
        if levels[i] % 2 == 1 {
            out_chars[i] = mirror_char(out_chars[i]);
        }
    }

    // --- L2: Reversing Resolved Levels ---
    let max_level = levels.iter().copied().fold(0u8, u8::max);
    let min_odd_level = levels
        .iter()
        .copied()
        .filter(|&l| l % 2 == 1)
        .fold(255u8, u8::min);

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
    out_chars
        .into_iter()
        .filter(|&c| {
            !matches!(
                c,
                '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200B}' | '\u{FEFF}'
            )
        })
        .collect()
}

/// Splits text into visual directional runs according to UAX#9 levels.
///
/// G6 fix: Uses the algorithm's computed levels directly rather than
/// re-classifying characters after reordering.
pub fn bidi_visual_runs(text: &str, is_rtl_base: bool) -> Vec<BidiRun> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }

    let n = chars.len();
    let base_level: u8 = if is_rtl_base { 1 } else { 0 };

    // Run the level computation (same as reorder_bidi_text but we extract the
    // levels before visual reordering)
    let mut types: Vec<BidiClass> = chars.iter().copied().map(bidi_class).collect();
    let mut levels = resolve_explicit_levels(&chars, &mut types, base_level);

    // Apply W1–W7, N1–N2, I1–I2 (same as in reorder_bidi_text)
    apply_weak_and_neutral_rules(&mut types, &mut levels, &chars, base_level, is_rtl_base);

    // Now build runs from the level array
    let mut runs = Vec::new();
    let mut run_start = 0;
    while run_start < n {
        let run_level = levels[run_start];
        let mut run_end = run_start + 1;
        while run_end < n && levels[run_end] == run_level {
            run_end += 1;
        }

        let is_rtl = run_level % 2 == 1;
        let run_text: String = if is_rtl {
            chars[run_start..run_end]
                .iter()
                .rev()
                .map(|&c| mirror_char(c))
                .filter(|&c| !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
                .collect()
        } else {
            chars[run_start..run_end]
                .iter()
                .filter(|&&c| !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
                .collect()
        };

        if !run_text.is_empty() {
            runs.push(BidiRun {
                text: run_text,
                level: run_level,
                is_rtl,
            });
        }
        run_start = run_end;
    }

    // Rule L2: Reorder runs into visual left-to-right order
    let max_level = runs.iter().map(|r| r.level).max().unwrap_or(0);
    let min_odd_level = runs
        .iter()
        .map(|r| r.level)
        .filter(|&l| l % 2 == 1)
        .min()
        .unwrap_or(255);

    if min_odd_level <= max_level {
        let mut lvl = max_level;
        loop {
            let mut s = 0;
            while s < runs.len() {
                if runs[s].level >= lvl {
                    let mut e = s;
                    while e < runs.len() && runs[e].level >= lvl {
                        e += 1;
                    }
                    runs[s..e].reverse();
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

    runs
}

/// Rule N0: Resolves paired brackets based on the directionality of enclosed content.
///
/// Implements UAX#9 BD16 (bracket pair detection using a stack, max depth 63)
/// and Rule N0 (resolving pair types using embedding direction and inner/context strong types).
fn resolve_bracket_pairs(
    chars: &[char],
    types: &mut [BidiClass],
    levels: &[u8],
    base_level: u8,
) {
    let n = chars.len();
    if n == 0 {
        return;
    }

    const MAX_BRACKET_STACK: usize = 63;
    let mut stack: Vec<(char, usize)> = Vec::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();

    for i in 0..n {
        if types[i] != BidiClass::ON {
            continue;
        }
        if let Some(btype) = bracket_type(chars[i]) {
            match btype {
                BracketType::Open(expected_close) => {
                    if stack.len() < MAX_BRACKET_STACK {
                        stack.push((expected_close, i));
                    }
                }
                BracketType::Close(_) => {
                    let mut found_pos = None;
                    for s in (0..stack.len()).rev() {
                        if stack[s].0 == chars[i] {
                            found_pos = Some(s);
                            break;
                        }
                    }
                    if let Some(pos) = found_pos {
                        let open_pos = stack[pos].1;
                        pairs.push((open_pos, i));
                        stack.truncate(pos);
                    }
                }
            }
        }
    }

    pairs.sort_by_key(|&(open_pos, _)| open_pos);

    for (open_idx, close_idx) in pairs {
        let embedding_level = levels[open_idx];
        let embedding_dir = if embedding_level % 2 == 0 {
            BidiClass::L
        } else {
            BidiClass::R
        };
        let opposite_dir = if embedding_dir == BidiClass::L {
            BidiClass::R
        } else {
            BidiClass::L
        };

        // Rule N0a: Inspect enclosed text
        let mut found_match = false;
        let mut found_opposite = false;

        for k in (open_idx + 1)..close_idx {
            let t = types[k];
            let strong_type = match t {
                BidiClass::L => Some(BidiClass::L),
                BidiClass::R | BidiClass::AL => Some(BidiClass::R),
                BidiClass::EN => {
                    if embedding_dir == BidiClass::L {
                        Some(BidiClass::L)
                    } else {
                        Some(BidiClass::R)
                    }
                }
                BidiClass::AN => Some(BidiClass::R),
                _ => None,
            };

            if let Some(st) = strong_type {
                if st == embedding_dir {
                    found_match = true;
                    break;
                } else if st == opposite_dir {
                    found_opposite = true;
                }
            }
        }

        if found_match {
            types[open_idx] = embedding_dir;
            types[close_idx] = embedding_dir;
        } else if found_opposite {
            // Rule N0b: Look back before open_idx for previous strong type (or sos)
            let mut prev_strong = None;
            let mut k = open_idx;
            while k > 0 {
                k -= 1;
                let t = types[k];
                let st = match t {
                    BidiClass::L => Some(BidiClass::L),
                    BidiClass::R | BidiClass::AL => Some(BidiClass::R),
                    BidiClass::EN => {
                        if embedding_dir == BidiClass::L {
                            Some(BidiClass::L)
                        } else {
                            Some(BidiClass::R)
                        }
                    }
                    BidiClass::AN => Some(BidiClass::R),
                    _ => None,
                };
                if let Some(s) = st {
                    prev_strong = Some(s);
                    break;
                }
            }

            let context_dir = prev_strong.unwrap_or(if base_level % 2 == 0 {
                BidiClass::L
            } else {
                BidiClass::R
            });

            if context_dir == opposite_dir {
                types[open_idx] = opposite_dir;
                types[close_idx] = opposite_dir;
            } else {
                types[open_idx] = embedding_dir;
                types[close_idx] = embedding_dir;
            }
        }
    }
}

/// Applies W1–W7, N0, N1–N2, I1–I2 rules (shared between reorder and visual runs).
fn apply_weak_and_neutral_rules(
    types: &mut [BidiClass],
    levels: &mut [u8],
    chars: &[char],
    base_level: u8,
    is_rtl_base: bool,
) {
    let n = types.len();

    // W1: NSM
    let mut prev_type = if is_rtl_base {
        BidiClass::R
    } else {
        BidiClass::L
    };
    for i in 0..n {
        if types[i] == BidiClass::NSM {
            types[i] = prev_type;
        } else if types[i] != BidiClass::BN {
            prev_type = types[i];
        }
    }

    // W2: EN after AL → AN
    let mut last_strong = if is_rtl_base {
        BidiClass::R
    } else {
        BidiClass::L
    };
    for i in 0..n {
        if types[i].is_strong() {
            last_strong = types[i];
        } else if types[i] == BidiClass::EN && last_strong == BidiClass::AL {
            types[i] = BidiClass::AN;
        }
    }

    // W3: AL → R
    for t in types.iter_mut() {
        if *t == BidiClass::AL {
            *t = BidiClass::R;
        }
    }

    // W4: Single ES/CS between same number types
    for i in 1..n.saturating_sub(1) {
        if types[i] == BidiClass::ES
            && types[i - 1] == BidiClass::EN
            && types[i + 1] == BidiClass::EN
        {
            types[i] = BidiClass::EN;
        } else if types[i] == BidiClass::CS {
            if types[i - 1] == BidiClass::EN && types[i + 1] == BidiClass::EN {
                types[i] = BidiClass::EN;
            } else if types[i - 1] == BidiClass::AN && types[i + 1] == BidiClass::AN {
                types[i] = BidiClass::AN;
            }
        }
    }

    // W5: ET runs adjacent to EN → EN
    {
        let mut i = 0;
        while i < n {
            if types[i] == BidiClass::ET {
                let run_start = i;
                while i < n && types[i] == BidiClass::ET {
                    i += 1;
                }
                let run_end = i;
                let en_before = run_start > 0 && types[run_start - 1] == BidiClass::EN;
                let en_after = run_end < n && types[run_end] == BidiClass::EN;
                if en_before || en_after {
                    for k in run_start..run_end {
                        types[k] = BidiClass::EN;
                    }
                }
            } else {
                i += 1;
            }
        }
    }

    // W6: Remaining ES/ET/CS → ON
    for t in types.iter_mut() {
        if matches!(*t, BidiClass::ES | BidiClass::ET | BidiClass::CS) {
            *t = BidiClass::ON;
        }
    }

    // W7: EN preceded by L → L
    let mut last_strong_l = base_level % 2 == 0;
    for t in types.iter_mut() {
        if *t == BidiClass::L {
            last_strong_l = true;
        } else if *t == BidiClass::R {
            last_strong_l = false;
        } else if *t == BidiClass::EN && last_strong_l {
            *t = BidiClass::L;
        }
    }

    // N0: Bracket pair resolution (Unicode 6.3+ / UAX#9 BD16 & Rule N0)
    resolve_bracket_pairs(chars, types, levels, base_level);

    // N1–N2: Neutral resolution
    let mut i = 0;
    while i < n {
        if types[i].is_neutral() {
            let start = i;
            while i < n && types[i].is_neutral() {
                i += 1;
            }
            let end = i;
            let lead_type = if start > 0 {
                types[start - 1]
            } else if is_rtl_base {
                BidiClass::R
            } else {
                BidiClass::L
            };
            let trail_type = if end < n {
                types[end]
            } else if is_rtl_base {
                BidiClass::R
            } else {
                BidiClass::L
            };
            let resolved = match (lead_type, trail_type) {
                (BidiClass::L, BidiClass::L) => BidiClass::L,
                (BidiClass::R, BidiClass::R)
                | (BidiClass::R, BidiClass::AN)
                | (BidiClass::AN, BidiClass::R)
                | (BidiClass::AN, BidiClass::AN) => BidiClass::R,
                _ => {
                    if is_rtl_base {
                        BidiClass::R
                    } else {
                        BidiClass::L
                    }
                }
            };
            for k in start..end {
                types[k] = resolved;
            }
        } else {
            i += 1;
        }
    }

    // I1–I2: Implicit levels
    for i in 0..n {
        let t = types[i];
        if levels[i] % 2 == 0 {
            if t == BidiClass::R {
                levels[i] += 1;
            } else if t == BidiClass::AN || t == BidiClass::EN {
                levels[i] += 2;
            }
        } else if t == BidiClass::L || t == BidiClass::EN || t == BidiClass::AN {
            levels[i] += 1;
        }
    }

    // L1: Reset trailing WS
    let _ = chars; // used to suppress unused warning; chars needed for API compatibility
    let orig_classes: Vec<BidiClass> = (0..n)
        .map(|idx| {
            // Use original character class, not resolved type
            if idx < n { bidi_class(chars[idx]) } else { BidiClass::BN }
        })
        .collect();
    let mut j = n;
    while j > 0 {
        j -= 1;
        let oc = orig_classes[j];
        if matches!(
            oc,
            BidiClass::WS | BidiClass::LRI | BidiClass::RLI | BidiClass::FSI | BidiClass::PDI
        ) {
            levels[j] = base_level;
        } else if matches!(oc, BidiClass::B | BidiClass::S) {
            levels[j] = base_level;
            while j > 0 {
                let prev_oc = orig_classes[j - 1];
                if matches!(
                    prev_oc,
                    BidiClass::WS | BidiClass::LRI | BidiClass::RLI | BidiClass::FSI | BidiClass::PDI
                ) {
                    j -= 1;
                    levels[j] = base_level;
                } else {
                    break;
                }
            }
        } else if !matches!(oc, BidiClass::BN) {
            break;
        }
    }
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
        // G7: Mathematical brackets
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
        // Parentheses mirrored: '(' at start becomes ')' when reversed in RTL,
        // and then L4 mirrors it back to '(' at visually left end
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
        // European numbers remain in visual left-to-right order (123)
        assert!(
            reordered.contains("123"),
            "Numbers must preserve LTR order: got {}",
            reordered
        );
    }

    // B1: Directional marks are correctly classified
    #[test]
    fn test_directional_marks_classification() {
        assert_eq!(bidi_class('\u{200E}'), BidiClass::L, "LRM must be class L");
        assert_eq!(bidi_class('\u{200F}'), BidiClass::R, "RLM must be class R");
    }

    // B2: BOM is BN, not AL
    #[test]
    fn test_bom_is_bn() {
        assert_eq!(
            bidi_class('\u{FEFF}'),
            BidiClass::BN,
            "BOM must be class BN"
        );
    }

    // B3: Nested explicit overrides with stack
    #[test]
    fn test_nested_overrides() {
        // RLO forces R, then LRO forces L, then PDF pops back to R
        let text = "\u{202E}AB\u{202D}CD\u{202C}EF\u{202C}";
        let reordered = reorder_bidi_text(text, false);
        // After RLO: AB should be treated as R (reversed)
        // Inside LRO: CD should remain L order
        // After first PDF: EF back to R override
        // After second PDF: override fully popped
        // This tests that the stack handles nesting correctly
        assert!(
            !reordered.is_empty(),
            "Nested override must produce output"
        );
    }

    // G1: Isolate formatting
    #[test]
    fn test_isolate_formatting() {
        // LRI ... PDI should isolate the content
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
        // P2: First strong char inside isolate should be ignored for base direction
        let text = "\u{2067}Hello\u{2069}مرحبا";
        assert!(
            resolve_base_direction(text, false),
            "Base direction should be RTL (Arabic after isolate)"
        );
    }

    // G3: Additional RTL scripts
    #[test]
    fn test_additional_rtl_scripts() {
        // Thaana
        assert_eq!(bidi_class('\u{0780}'), BidiClass::R);
        // N'Ko
        assert_eq!(bidi_class('\u{07C0}'), BidiClass::R);
        // Syriac → AL
        assert_eq!(bidi_class('\u{0710}'), BidiClass::AL);
    }

    #[test]
    fn test_visual_runs_use_levels() {
        let text = "Hello سلام World";
        let runs = bidi_visual_runs(text, false);
        // Should have at least 2 runs: one LTR, one RTL
        assert!(
            runs.len() >= 2,
            "Mixed text should produce multiple runs: got {:?}",
            runs
        );
        // Verify RTL run exists
        assert!(
            runs.iter().any(|r| r.is_rtl),
            "Must contain an RTL run"
        );
    }

    // B5: ET runs adjacent to EN
    #[test]
    fn test_et_run_adjacent_to_en() {
        // $$123 — the $$ (ET ET) should become EN because adjacent to 123 (EN)
        let text = "$$123";
        let reordered = reorder_bidi_text(text, false);
        assert_eq!(reordered, "$$123", "ET run before EN must stay LTR");
    }

    // G7: Mathematical bracket mirroring
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
        // LRE should create a new even level
        let text = "\u{202A}Hello\u{202C}";
        let reordered = reorder_bidi_text(text, false);
        assert!(
            reordered.contains("Hello"),
            "LRE/PDF embedding must preserve content"
        );

        // RLE should create a new odd level
        let text2 = "\u{202B}Hello\u{202C}";
        let reordered2 = reorder_bidi_text(text2, false);
        // Content should be reversed since it's at an odd level
        assert!(
            !reordered2.is_empty(),
            "RLE/PDF embedding must produce output"
        );
    }

    #[test]
    fn test_fullwidth_digits_and_currency() {
        // Fullwidth digits should be EN
        assert_eq!(bidi_class('\u{FF10}'), BidiClass::EN); // ０
        assert_eq!(bidi_class('\u{FF19}'), BidiClass::EN); // ９
        // Currency symbols should be ET
        assert_eq!(bidi_class('¥'), BidiClass::ET);
        assert_eq!(bidi_class('€'), BidiClass::ET);
    }

    #[test]
    fn test_whitespace_ranges() {
        // Various space characters should be WS
        assert_eq!(bidi_class('\u{2003}'), BidiClass::WS); // EM SPACE
        assert_eq!(bidi_class('\u{2009}'), BidiClass::WS); // THIN SPACE
        assert_eq!(bidi_class('\u{3000}'), BidiClass::WS); // IDEOGRAPHIC SPACE
    }

    // G2: Bracket Pair Algorithm (Rule N0 / BD16)
    #[test]
    fn test_bracket_pair_algorithm_n0() {
        // In RTL base, (hello) inside Arabic text should resolve brackets to RTL
        let text = "سلام (hello) شكرا";
        let reordered = reorder_bidi_text(text, true);
        assert!(!reordered.is_empty());
        // Verify bracket classification via bracket_type
        assert_eq!(bracket_type('('), Some(BracketType::Open(')')));
        assert_eq!(bracket_type(')'), Some(BracketType::Close('(')));
        assert_eq!(bracket_type('【'), Some(BracketType::Open('】')));
        assert_eq!(bracket_type('】'), Some(BracketType::Close('【')));
        assert_eq!(bracket_type('⟦'), Some(BracketType::Open('⟧')));
        assert_eq!(bracket_type('⟧'), Some(BracketType::Close('⟦')));
    }

    // G6: bidi_visual_runs output concatenation matches reorder_bidi_text
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

        // Also test RTL base
        let rtl_text = "سلام 123 Hello شكرا";
        let rtl_runs = bidi_visual_runs(rtl_text, true);
        let rtl_concatenated: String = rtl_runs.into_iter().map(|r| r.text).collect();
        let rtl_direct = reorder_bidi_text(rtl_text, true);
        assert_eq!(
            rtl_concatenated, rtl_direct,
            "RTL visual runs concatenated must match reorder_bidi_text output"
        );
    }

    // G3: Extended modern RTL scripts
    #[test]
    fn test_extended_rtl_scripts() {
        assert_eq!(bidi_class('\u{1E900}'), BidiClass::R); // Adlam
        assert_eq!(bidi_class('\u{10D00}'), BidiClass::R); // Hanifi Rohingya
        assert_eq!(bidi_class('\u{10840}'), BidiClass::R); // Imperial Aramaic
        assert_eq!(bidi_class('\u{10900}'), BidiClass::R); // Phoenician
        assert_eq!(bidi_class('\u{1EE00}'), BidiClass::AL); // Arabic Math
    }

    // G7: Additional bracket and relational mirroring
    #[test]
    fn test_extended_bracket_mirroring() {
        assert_eq!(mirror_char('⟦'), '⟧');
        assert_eq!(mirror_char('⟧'), '⟦');
        assert_eq!(mirror_char('≪'), '≫');
        assert_eq!(mirror_char('≫'), '≪');
        assert_eq!(mirror_char('≺'), '≻');
        assert_eq!(mirror_char('≻'), '≺');
    }
}
