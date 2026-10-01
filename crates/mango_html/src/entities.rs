//! HTML character entity reference resolution.
//!
//! Implements named entity lookup and numeric (decimal / hexadecimal)
//! character reference decoding according to the HTML specification.

/// Resolves a named character entity (without the leading `&` or trailing `;`).
///
/// Returns the replacement string if recognized, or `None`.
pub fn resolve_named_entity(name: &str) -> Option<&'static str> {
    match name {
        // Essential XML / HTML entities
        "amp" => Some("&"),
        "lt" => Some("<"),
        "gt" => Some(">"),
        "quot" => Some("\""),
        "apos" => Some("'"),

        // Whitespace and formatting
        "nbsp" => Some("\u{00A0}"),
        "ensp" => Some("\u{2002}"),
        "emsp" => Some("\u{2003}"),
        "thinsp" => Some("\u{2009}"),
        "zwnj" => Some("\u{200C}"),
        "zwj" => Some("\u{200D}"),
        "lrm" => Some("\u{200E}"),
        "rlm" => Some("\u{200F}"),

        // Punctuation and typography
        "ndash" => Some("\u{2013}"),
        "mdash" => Some("\u{2014}"),
        "lsquo" => Some("\u{2018}"),
        "rsquo" => Some("\u{2019}"),
        "sbquo" => Some("\u{201A}"),
        "ldquo" => Some("\u{201C}"),
        "rdquo" => Some("\u{201D}"),
        "bdquo" => Some("\u{201E}"),
        "dagger" => Some("\u{2020}"),
        "Dagger" => Some("\u{2021}"),
        "bull" => Some("\u{2022}"),
        "hellip" => Some("\u{2026}"),
        "permil" => Some("\u{2030}"),
        "prime" => Some("\u{2032}"),
        "Prime" => Some("\u{2033}"),
        "lsaquo" => Some("\u{2039}"),
        "rsaquo" => Some("\u{203A}"),
        "oline" => Some("\u{203E}"),
        "frasl" => Some("\u{2044}"),

        // Symbols and currency
        "euro" => Some("\u{20AC}"),
        "pound" => Some("\u{00A3}"),
        "yen" => Some("\u{00A5}"),
        "cent" => Some("\u{00A2}"),
        "curren" => Some("\u{00A4}"),
        "copy" => Some("\u{00A9}"),
        "reg" => Some("\u{00AE}"),
        "trade" => Some("\u{2122}"),
        "sect" => Some("\u{00A7}"),
        "deg" => Some("\u{00B0}"),
        "plusmn" => Some("\u{00B1}"),
        "sup1" => Some("\u{00B9}"),
        "sup2" => Some("\u{00B2}"),
        "sup3" => Some("\u{00B3}"),
        "micro" => Some("\u{00B5}"),
        "para" => Some("\u{00B6}"),
        "middot" => Some("\u{00B7}"),
        "frac14" => Some("\u{00BC}"),
        "frac12" => Some("\u{00BD}"),
        "frac34" => Some("\u{00BE}"),
        "iquest" => Some("\u{00BF}"),
        "iexcl" => Some("\u{00A1}"),
        "laquo" => Some("\u{00AB}"),
        "raquo" => Some("\u{00BB}"),
        "shy" => Some("\u{00AD}"),
        "macr" => Some("\u{00AF}"),

        // Math symbols
        "forall" => Some("\u{2200}"),
        "part" => Some("\u{2202}"),
        "exist" => Some("\u{2203}"),
        "empty" => Some("\u{2205}"),
        "nabla" => Some("\u{2207}"),
        "isin" => Some("\u{2208}"),
        "notin" => Some("\u{2209}"),
        "ni" => Some("\u{220B}"),
        "prod" => Some("\u{220F}"),
        "sum" => Some("\u{2211}"),
        "minus" => Some("\u{2212}"),
        "lowast" => Some("\u{2217}"),
        "radic" => Some("\u{221A}"),
        "prop" => Some("\u{221D}"),
        "infin" => Some("\u{221E}"),
        "ang" => Some("\u{2220}"),
        "and" => Some("\u{2227}"),
        "or" => Some("\u{2228}"),
        "cap" => Some("\u{2229}"),
        "cup" => Some("\u{222A}"),
        "int" => Some("\u{222B}"),
        "there4" => Some("\u{2234}"),
        "sim" => Some("\u{223C}"),
        "cong" => Some("\u{2245}"),
        "asymp" => Some("\u{2248}"),
        "ne" => Some("\u{2260}"),
        "equiv" => Some("\u{2261}"),
        "le" => Some("\u{2264}"),
        "ge" => Some("\u{2265}"),
        "sub" => Some("\u{2282}"),
        "sup" => Some("\u{2283}"),
        "nsub" => Some("\u{2284}"),
        "sube" => Some("\u{2286}"),
        "supe" => Some("\u{2287}"),
        "oplus" => Some("\u{2295}"),
        "otimes" => Some("\u{2297}"),
        "perp" => Some("\u{22A5}"),
        "sdot" => Some("\u{22C5}"),
        "times" => Some("\u{00D7}"),
        "divide" => Some("\u{00F7}"),

        // Greek letters (common subset)
        "alpha" => Some("α"),
        "beta" => Some("β"),
        "gamma" => Some("γ"),
        "delta" => Some("δ"),
        "epsilon" => Some("ε"),
        "zeta" => Some("ζ"),
        "eta" => Some("η"),
        "theta" => Some("θ"),
        "iota" => Some("ι"),
        "kappa" => Some("κ"),
        "lambda" => Some("λ"),
        "mu" => Some("μ"),
        "nu" => Some("ν"),
        "xi" => Some("ξ"),
        "omicron" => Some("ο"),
        "pi" => Some("π"),
        "rho" => Some("ρ"),
        "sigma" => Some("σ"),
        "tau" => Some("τ"),
        "upsilon" => Some("υ"),
        "phi" => Some("φ"),
        "chi" => Some("χ"),
        "psi" => Some("ψ"),
        "omega" => Some("ω"),

        "Alpha" => Some("Α"),
        "Beta" => Some("Β"),
        "Gamma" => Some("Γ"),
        "Delta" => Some("Δ"),
        "Epsilon" => Some("Ε"),
        "Zeta" => Some("Ζ"),
        "Eta" => Some("Η"),
        "Theta" => Some("Θ"),
        "Iota" => Some("Ι"),
        "Kappa" => Some("Κ"),
        "Lambda" => Some("Λ"),
        "Mu" => Some("Μ"),
        "Nu" => Some("Ν"),
        "Xi" => Some("Ξ"),
        "Omicron" => Some("Ο"),
        "Pi" => Some("Π"),
        "Rho" => Some("Ρ"),
        "Sigma" => Some("Σ"),
        "Tau" => Some("Τ"),
        "Upsilon" => Some("Υ"),
        "Phi" => Some("Φ"),
        "Chi" => Some("Χ"),
        "Psi" => Some("Ψ"),
        "Omega" => Some("Ω"),

        // Latin accents (common)
        "Agrave" => Some("À"),
        "Aacute" => Some("Á"),
        "Acirc" => Some("Â"),
        "Atilde" => Some("Ã"),
        "Auml" => Some("Ä"),
        "Aring" => Some("Å"),
        "AElig" => Some("Æ"),
        "Ccedil" => Some("Ç"),
        "Egrave" => Some("È"),
        "Eacute" => Some("É"),
        "Ecirc" => Some("Ê"),
        "Euml" => Some("Ë"),
        "Igrave" => Some("Ì"),
        "Iacute" => Some("Í"),
        "Icirc" => Some("Î"),
        "Iuml" => Some("Ï"),
        "ETH" => Some("Ð"),
        "Ntilde" => Some("Ñ"),
        "Ograve" => Some("Ò"),
        "Oacute" => Some("Ó"),
        "Ocirc" => Some("Ô"),
        "Otilde" => Some("Õ"),
        "Ouml" => Some("Ö"),
        "Oslash" => Some("Ø"),
        "Ugrave" => Some("Ù"),
        "Uacute" => Some("Ú"),
        "Ucirc" => Some("Û"),
        "Uuml" => Some("Ü"),
        "Yacute" => Some("Ý"),
        "THORN" => Some("Þ"),
        "szlig" => Some("ß"),
        "agrave" => Some("à"),
        "aacute" => Some("á"),
        "acirc" => Some("â"),
        "atilde" => Some("ã"),
        "auml" => Some("ä"),
        "aring" => Some("å"),
        "aelig" => Some("æ"),
        "ccedil" => Some("ç"),
        "egrave" => Some("è"),
        "eacute" => Some("é"),
        "ecirc" => Some("ê"),
        "euml" => Some("ë"),
        "igrave" => Some("ì"),
        "iacute" => Some("í"),
        "icirc" => Some("î"),
        "iuml" => Some("ï"),
        "eth" => Some("ð"),
        "ntilde" => Some("ñ"),
        "ograve" => Some("ò"),
        "oacute" => Some("ó"),
        "ocirc" => Some("ô"),
        "otilde" => Some("õ"),
        "ouml" => Some("ö"),
        "oslash" => Some("ø"),
        "ugrave" => Some("ù"),
        "uacute" => Some("ú"),
        "ucirc" => Some("û"),
        "uuml" => Some("ü"),
        "yacute" => Some("ý"),
        "thorn" => Some("þ"),
        "yuml" => Some("ÿ"),

        _ => None,
    }
}

/// Decodes numeric character references (e.g. `&#65;` or `&#x41;`).
///
/// Replaces invalid code points with the Unicode replacement character `\u{FFFD}`.
pub fn resolve_numeric_entity(raw: &str) -> Option<char> {
    let (digits, radix) = if let Some(hex) = raw.strip_prefix('x').or_else(|| raw.strip_prefix('X')) {
        (hex, 16)
    } else {
        (raw, 10)
    };

    let code_point = u32::from_str_radix(digits, radix).ok()?;

    // WHATWG HTML replacement table for Windows-1252 compatibility
    let mapped_code_point = match code_point {
        0x00 => 0xFFFD, // null replacement
        0x80 => 0x20AC, // Euro
        0x82 => 0x201A, // Single Low-9 Quotation Mark
        0x83 => 0x0192, // Latin Small Letter F with Hook
        0x84 => 0x201E, // Double Low-9 Quotation Mark
        0x85 => 0x2026, // Horizontal Ellipsis
        0x86 => 0x2020, // Dagger
        0x87 => 0x2021, // Double Dagger
        0x88 => 0x02C6, // Modifier Letter Circumflex Accent
        0x89 => 0x2030, // Per Mille Sign
        0x8A => 0x0160, // Latin Capital Letter S with Caron
        0x8B => 0x2039, // Single Left-Pointing Angle Quotation Mark
        0x8C => 0x0152, // Latin Capital Ligature OE
        0x8E => 0x017D, // Latin Capital Letter Z with Caron
        0x91 => 0x2018, // Left Single Quotation Mark
        0x92 => 0x2019, // Right Single Quotation Mark
        0x93 => 0x201C, // Left Double Quotation Mark
        0x94 => 0x201D, // Right Double Quotation Mark
        0x95 => 0x2022, // Bullet
        0x96 => 0x2013, // En Dash
        0x97 => 0x2014, // Em Dash
        0x98 => 0x02DC, // Small Tilde
        0x99 => 0x2122, // Trade Mark Sign
        0x9A => 0x0161, // Latin Small Letter S with Caron
        0x9B => 0x203A, // Single Right-Pointing Angle Quotation Mark
        0x9C => 0x0153, // Latin Small Ligature OE
        0x9E => 0x017E, // Latin Small Letter Z with Caron
        0x9F => 0x0178, // Latin Capital Letter Y with Diaeresis
        other => other,
    };

    char::from_u32(mapped_code_point).or(Some('\u{FFFD}'))
}

/// Decodes all HTML entities in an input string.
///
/// Handles `&name;`, `&#123;`, and `&#x1F600;`. Unrecognized entities
/// are left as-is.
pub fn decode_entities(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();

    while let Some((_, ch)) = chars.next() {
        if ch != '&' {
            result.push(ch);
            continue;
        }

        // We encountered '&' — try to match an entity
        let mut entity_content = String::new();
        let mut matched = false;

        while let Some(&(_, next_ch)) = chars.peek() {
            if next_ch == ';' {
                chars.next(); // consume ';'
                matched = true;
                break;
            }
            if next_ch.is_alphanumeric() || next_ch == '#' {
                chars.next();
                entity_content.push(next_ch);
                if entity_content.len() > 32 {
                    // Entity names are not longer than ~32 chars
                    break;
                }
            } else {
                break;
            }
        }

        if matched && !entity_content.is_empty() {
            if let Some(numeric_part) = entity_content.strip_prefix('#') {
                if let Some(decoded_ch) = resolve_numeric_entity(numeric_part) {
                    result.push(decoded_ch);
                    continue;
                }
            } else if let Some(replacement) = resolve_named_entity(&entity_content) {
                result.push_str(replacement);
                continue;
            }
            // Failed to resolve: write out raw original
            result.push('&');
            result.push_str(&entity_content);
            result.push(';');
        } else {
            // Not a complete entity
            result.push('&');
            result.push_str(&entity_content);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_named_entities() {
        assert_eq!(decode_entities("&amp;"), "&");
        assert_eq!(decode_entities("&lt;"), "<");
        assert_eq!(decode_entities("&gt;"), ">");
        assert_eq!(decode_entities("&quot;"), "\"");
        assert_eq!(decode_entities("&apos;"), "'");
    }

    #[test]
    fn test_numeric_decimal() {
        assert_eq!(decode_entities("&#65;"), "A");
        assert_eq!(decode_entities("&#97;"), "a");
    }

    #[test]
    fn test_numeric_hex() {
        assert_eq!(decode_entities("&#x41;"), "A");
        assert_eq!(decode_entities("&#x1F600;"), "😀");
    }

    #[test]
    fn test_mixed_text() {
        let raw = "Mango &amp; Dillo &lt;3 HTML5 &copy; 2026";
        let expected = "Mango & Dillo <3 HTML5 © 2026";
        assert_eq!(decode_entities(raw), expected);
    }

    #[test]
    fn test_unclosed_or_invalid_entities() {
        assert_eq!(decode_entities("&unknown;"), "&unknown;");
        assert_eq!(decode_entities("Fish & Chips"), "Fish & Chips");
        assert_eq!(decode_entities("x && y"), "x && y");
    }
}
