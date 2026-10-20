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
