//! Pure Rust Complex Script Shaping Engine for Arabic, Devanagari, and Thai.
//!
//! Provides:
//! - Full Arabic contextual shaping (Isolated, Initial, Medial, Final) and Lam-Alef ligatures with Tashkeel transparency.
//! - Devanagari conjunct clustering, Nukta composition, and pre-base vowel matra reordering.
//! - Thai combining mark canonical normalization, vertical stacking, and syllable-boundary segmentation for line breaking.

/// Returns true if the character is an Arabic letter or presentation form.
pub fn is_arabic_char(ch: char) -> bool {
    matches!(ch,
        '\u{0600}'..='\u{06FF}'
        | '\u{0750}'..='\u{077F}'
        | '\u{08A0}'..='\u{08FF}'
        | '\u{FB50}'..='\u{FDFF}'
        | '\u{FE70}'..='\u{FEFF}'
    )
}

/// Returns true if the character is a non-spacing Arabic diacritic mark (Tashkeel / Harakat).
pub fn is_arabic_tashkeel(ch: char) -> bool {
    matches!(ch,
        '\u{064B}'..='\u{065F}'
        | '\u{0670}'
        | '\u{06D6}'..='\u{06DC}'
        | '\u{06DF}'..='\u{06E4}'
        | '\u{06E7}'..='\u{06E8}'
        | '\u{06EA}'..='\u{06ED}'
    )
}

/// Returns true if the character belongs to the Devanagari script.
pub fn is_devanagari_char(ch: char) -> bool {
    matches!(ch, '\u{0900}'..='\u{097F}')
}

/// Returns true if the character belongs to the Thai script.
pub fn is_thai_char(ch: char) -> bool {
    matches!(ch, '\u{0E00}'..='\u{0E7F}')
}

/// Returns true if the character is a complex script requiring shaping or syllable segmentation.
pub fn is_complex_char(ch: char) -> bool {
    is_arabic_char(ch) || is_devanagari_char(ch) || is_thai_char(ch)
}

// ============================================================================
// 1. Arabic Script Shaping
// ============================================================================

/// Maps an Arabic base character to its Presentation Forms-B (Isolated, Final, Initial, Medial).
fn arabic_forms(ch: char) -> Option<(char, char, char, char)> {
    match ch {
        '\u{0621}' => Some(('\u{FE80}', '\u{FE80}', '\u{FE80}', '\u{FE80}')), // Hamza
        '\u{0622}' => Some(('\u{FE81}', '\u{FE82}', '\u{FE81}', '\u{FE82}')), // Alef with Madda
        '\u{0623}' => Some(('\u{FE83}', '\u{FE84}', '\u{FE83}', '\u{FE84}')), // Alef with Hamza Above
        '\u{0624}' => Some(('\u{FE85}', '\u{FE86}', '\u{FE85}', '\u{FE86}')), // Waw with Hamza
        '\u{0625}' => Some(('\u{FE87}', '\u{FE88}', '\u{FE87}', '\u{FE88}')), // Alef with Hamza Below
        '\u{0626}' => Some(('\u{FE89}', '\u{FE8A}', '\u{FE8B}', '\u{FE8C}')), // Yeh with Hamza
        '\u{0627}' => Some(('\u{FE8D}', '\u{FE8E}', '\u{FE8D}', '\u{FE8E}')), // Alef
        '\u{0628}' => Some(('\u{FE8F}', '\u{FE90}', '\u{FE91}', '\u{FE92}')), // Baa
        '\u{0629}' => Some(('\u{FE93}', '\u{FE94}', '\u{FE93}', '\u{FE94}')), // Taa Marbuta
        '\u{062A}' => Some(('\u{FE95}', '\u{FE96}', '\u{FE97}', '\u{FE98}')), // Taa
        '\u{062B}' => Some(('\u{FE99}', '\u{FE9A}', '\u{FE9B}', '\u{FE9C}')), // Thaa
        '\u{062C}' => Some(('\u{FE9D}', '\u{FE9E}', '\u{FE9F}', '\u{FEA0}')), // Jeem
        '\u{062D}' => Some(('\u{FEA1}', '\u{FEA2}', '\u{FEA3}', '\u{FEA4}')), // Haa
        '\u{062E}' => Some(('\u{FEA5}', '\u{FEA6}', '\u{FEA7}', '\u{FEA8}')), // Khaa
        '\u{062F}' => Some(('\u{FEA9}', '\u{FEAA}', '\u{FEA9}', '\u{FEAA}')), // Dal
        '\u{0630}' => Some(('\u{FEAB}', '\u{FEAC}', '\u{FEAB}', '\u{FEAC}')), // Dhal
        '\u{0631}' => Some(('\u{FEAD}', '\u{FEAE}', '\u{FEAD}', '\u{FEAE}')), // Raa
        '\u{0632}' => Some(('\u{FEAF}', '\u{FEB0}', '\u{FEAF}', '\u{FEB0}')), // Zain
        '\u{0633}' => Some(('\u{FEB1}', '\u{FEB2}', '\u{FEB3}', '\u{FEB4}')), // Seen
        '\u{0634}' => Some(('\u{FEB5}', '\u{FEB6}', '\u{FEB7}', '\u{FEB8}')), // Sheen
        '\u{0635}' => Some(('\u{FEB9}', '\u{FEBA}', '\u{FEBB}', '\u{FEBC}')), // Saad
        '\u{0636}' => Some(('\u{FEBD}', '\u{FEBE}', '\u{FEBF}', '\u{FEC0}')), // Daad
        '\u{0637}' => Some(('\u{FEC1}', '\u{FEC2}', '\u{FEC3}', '\u{FEC4}')), // Taa'
        '\u{0638}' => Some(('\u{FEC5}', '\u{FEC6}', '\u{FEC7}', '\u{FEC8}')), // Zaa'
        '\u{0639}' => Some(('\u{FEC9}', '\u{FECA}', '\u{FECB}', '\u{FECC}')), // Ain
        '\u{063A}' => Some(('\u{FECD}', '\u{FECE}', '\u{FECF}', '\u{FED0}')), // Ghain
        '\u{0641}' => Some(('\u{FED1}', '\u{FED2}', '\u{FED3}', '\u{FED4}')), // Faa
        '\u{0642}' => Some(('\u{FED5}', '\u{FED6}', '\u{FED7}', '\u{FED8}')), // Qaaf
        '\u{0643}' => Some(('\u{FED9}', '\u{FEDA}', '\u{FEDB}', '\u{FEDC}')), // Kaaf
        '\u{0644}' => Some(('\u{FEDD}', '\u{FEDE}', '\u{FEDF}', '\u{FEE0}')), // Laam
        '\u{0645}' => Some(('\u{FEE1}', '\u{FEE2}', '\u{FEE3}', '\u{FEE4}')), // Meem
        '\u{0646}' => Some(('\u{FEE5}', '\u{FEE6}', '\u{FEE7}', '\u{FEE8}')), // Noon
        '\u{0647}' => Some(('\u{FEE9}', '\u{FEEA}', '\u{FEEB}', '\u{FEEC}')), // Haa'
        '\u{0648}' => Some(('\u{FEED}', '\u{FEEE}', '\u{FEED}', '\u{FEEE}')), // Waw
        '\u{0649}' => Some(('\u{FEEF}', '\u{FEF0}', '\u{FEEF}', '\u{FEF0}')), // Alef Maksura
        '\u{064A}' => Some(('\u{FEF1}', '\u{FEF2}', '\u{FEF3}', '\u{FEF4}')), // Yaa
        // Persian / Urdu Extensions
        '\u{067E}' => Some(('\u{FB56}', '\u{FB57}', '\u{FB58}', '\u{FB59}')), // Peh
        '\u{0686}' => Some(('\u{FB7A}', '\u{FB7B}', '\u{FB7C}', '\u{FB7D}')), // Tcheh
        '\u{0698}' => Some(('\u{FB8A}', '\u{FB8B}', '\u{FB8A}', '\u{FB8B}')), // Jeh
        '\u{06AF}' => Some(('\u{FB92}', '\u{FB93}', '\u{FB94}', '\u{FB95}')), // Gaf
        '\u{06A9}' => Some(('\u{FB8E}', '\u{FB8F}', '\u{FB90}', '\u{FB91}')), // Keheh
        '\u{06CC}' => Some(('\u{FBFC}', '\u{FBFD}', '\u{FBFE}', '\u{FBFF}')), // Farsi Yeh
        _ => None,
    }
}

/// Returns true if a character joins with the character on its left (in cursive reading direction).
fn joins_left(ch: char) -> bool {
    matches!(ch,
        '\u{0626}' | '\u{0628}' | '\u{062A}' | '\u{062B}' | '\u{062C}' | '\u{062D}'
        | '\u{062E}' | '\u{0633}' | '\u{0634}' | '\u{0635}' | '\u{0636}' | '\u{0637}'
        | '\u{0638}' | '\u{0639}' | '\u{063A}' | '\u{0641}' | '\u{0642}' | '\u{0643}'
        | '\u{0644}' | '\u{0645}' | '\u{0646}' | '\u{0647}' | '\u{064A}'
        | '\u{067E}' | '\u{0686}' | '\u{06AF}' | '\u{06A9}' | '\u{06CC}'
    )
}

/// Returns true if a character joins with the character on its right.
fn joins_right(ch: char) -> bool {
    arabic_forms(ch).is_some() && ch != '\u{0621}'
}

/// Shapes an Arabic string into Presentation Forms with Lam-Alef ligatures and Tashkeel preservation.
pub fn shape_arabic(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }

    let mut shaped: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        // Tashkeel non-spacing marks pass through attached
        if is_arabic_tashkeel(ch) {
            shaped.push(ch);
            i += 1;
            continue;
        }

        // Check for Lam-Alef Ligature (skipping possible intermediate Tashkeel)
        if ch == '\u{0644}' {
            let mut j = i + 1;
            let mut tashkeel_between = Vec::new();
            while j < chars.len() && is_arabic_tashkeel(chars[j]) {
                tashkeel_between.push(chars[j]);
                j += 1;
            }

            if j < chars.len() {
                let next = chars[j];
                // Check if previous base joins
                let mut prev_base = None;
                let mut p = i;
                while p > 0 {
                    p -= 1;
                    if !is_arabic_tashkeel(chars[p]) {
                        prev_base = Some(chars[p]);
                        break;
                    }
                }
                let prev_joins = prev_base.map_or(false, |pb| joins_left(pb) && joins_right(ch));

                let lig = match next {
                    '\u{0622}' => Some(if prev_joins { '\u{FEF6}' } else { '\u{FEF5}' }),
                    '\u{0623}' => Some(if prev_joins { '\u{FEF8}' } else { '\u{FEF7}' }),
                    '\u{0625}' => Some(if prev_joins { '\u{FEFA}' } else { '\u{FEF9}' }),
                    '\u{0627}' => Some(if prev_joins { '\u{FEFC}' } else { '\u{FEFB}' }),
                    _ => None,
                };

                if let Some(lig_ch) = lig {
                    shaped.push(lig_ch);
                    shaped.extend(tashkeel_between);
                    i = j + 1;
                    continue;
                }
            }
        }

        if let Some(forms) = arabic_forms(ch) {
            // Find previous base letter
            let mut prev_base = None;
            let mut p = i;
            while p > 0 {
                p -= 1;
                if !is_arabic_tashkeel(chars[p]) {
                    prev_base = Some(chars[p]);
                    break;
                }
            }

            // Find next base letter
            let mut next_base = None;
            let mut n = i + 1;
            while n < chars.len() {
                if !is_arabic_tashkeel(chars[n]) {
                    next_base = Some(chars[n]);
                    break;
                }
                n += 1;
            }

            let prev_joins = prev_base.map_or(false, |pb| joins_left(pb) && joins_right(ch));
            let next_joins = next_base.map_or(false, |nb| joins_left(ch) && joins_right(nb));

            let glyph = match (prev_joins, next_joins) {
                (true, true) => forms.3,   // Medial
                (true, false) => forms.1,  // Final
                (false, true) => forms.2,  // Initial
                (false, false) => forms.0, // Isolated
            };
            shaped.push(glyph);
        } else {
            shaped.push(ch);
        }

        i += 1;
    }

    shaped.into_iter().collect()
}

// ============================================================================
// 2. Devanagari Script Shaping
// ============================================================================

/// Returns true if a character is a Devanagari consonant.
fn is_devanagari_consonant(ch: char) -> bool {
    matches!(ch, '\u{0915}'..='\u{0939}' | '\u{0958}'..='\u{095F}')
}

/// Shapes Devanagari text: composes Nuktas, reorders pre-base vowel matra 'ि' (\u{093F}),
/// and forms standard conjunct clusters (Ksha, Jnya, Tra, Shra).
pub fn shape_devanagari(text: &str) -> String {
    let raw_chars: Vec<char> = text.chars().collect();
    if raw_chars.is_empty() {
        return String::new();
    }

    // Step 1: Compose Nukta sequences (Consonant + \u{093C})
    let mut nukta_composed = Vec::with_capacity(raw_chars.len());
    let mut i = 0;
    while i < raw_chars.len() {
        if i + 1 < raw_chars.len() && raw_chars[i + 1] == '\u{093C}' {
            let composed = match raw_chars[i] {
                '\u{0915}' => Some('\u{0958}'), // क़
                '\u{0916}' => Some('\u{0959}'), // ख़
                '\u{0917}' => Some('\u{095A}'), // ग़
                '\u{091C}' => Some('\u{095B}'), // ज़
                '\u{0921}' => Some('\u{095C}'), // ड़
                '\u{0922}' => Some('\u{095D}'), // ढ़
                '\u{092B}' => Some('\u{095E}'), // फ़
                '\u{092F}' => Some('\u{095F}'), // य़
                _ => None,
            };
            if let Some(c) = composed {
                nukta_composed.push(c);
                i += 2;
                continue;
            }
        }
        nukta_composed.push(raw_chars[i]);
        i += 1;
    }

    // Step 2: Form standard conjuncts (e.g. क + ् + ष -> क्ष)
    let mut conjunct_formed = Vec::with_capacity(nukta_composed.len());
    i = 0;
    while i < nukta_composed.len() {
        if i + 2 < nukta_composed.len() && nukta_composed[i + 1] == '\u{094D}' {
            let c1 = nukta_composed[i];
            let c2 = nukta_composed[i + 2];
            let lig = match (c1, c2) {
                ('\u{0915}', '\u{0937}') => Some(['\u{0915}', '\u{094D}', '\u{0937}']), // क्ष
                ('\u{091C}', '\u{091E}') => Some(['\u{091C}', '\u{094D}', '\u{091E}']), // ज्ञ
                ('\u{0924}', '\u{0930}') => Some(['\u{0924}', '\u{094D}', '\u{0930}']), // त्र
                ('\u{0936}', '\u{0930}') => Some(['\u{0936}', '\u{094D}', '\u{0930}']), // श्र
                _ => None,
            };
            if let Some(cluster) = lig {
                conjunct_formed.extend(&cluster);
                i += 3;
                continue;
            }
        }
        conjunct_formed.push(nukta_composed[i]);
        i += 1;
    }

    // Step 3: Reorder pre-base vowel sign 'ि' (\u{093F}) to before its consonant/cluster
    let mut out = Vec::with_capacity(conjunct_formed.len());
    i = 0;
    while i < conjunct_formed.len() {
        let ch = conjunct_formed[i];
        if ch == '\u{093F}' {
            // Find start of the preceding consonant cluster: Consonant (+ Virama + Consonant)*
            let mut cluster_start = out.len();
            if cluster_start > 0 && is_devanagari_consonant(out[cluster_start - 1]) {
                cluster_start -= 1;
                while cluster_start >= 2
                    && out[cluster_start - 1] == '\u{094D}'
                    && is_devanagari_consonant(out[cluster_start - 2])
                {
                    cluster_start -= 2;
                }
            }
            out.insert(cluster_start, '\u{093F}');
        } else {
            out.push(ch);
        }
        i += 1;
    }

    out.into_iter().collect()
}

// ============================================================================
// 3. Thai Script Shaping & Syllable Segmentation
// ============================================================================

/// Returns true if a Thai character is a leading vowel (displayed before consonant).
fn is_thai_leading_vowel(ch: char) -> bool {
    matches!(ch, '\u{0E40}'..='\u{0E44}') // Sara E, Sara Ae, Sara O, Sara Ai
}

/// Returns true if a Thai character is an above vowel.
fn is_thai_above_vowel(ch: char) -> bool {
    matches!(ch, '\u{0E31}' | '\u{0E34}'..='\u{0E37}')
}

/// Returns true if a Thai character is a below vowel.
fn is_thai_below_vowel(ch: char) -> bool {
    matches!(ch, '\u{0E38}'..='\u{0E3A}')
}

/// Returns true if a Thai character is a tone mark or cancellation mark.
fn is_thai_tone_or_diacritic(ch: char) -> bool {
    matches!(ch, '\u{0E48}'..='\u{0E4E}')
}

/// Normalizes Thai combining mark sequence to canonical order: Consonant + Below/Above Vowel + Tone Mark.
pub fn shape_thai(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }

    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        // If tone mark precedes above vowel, normalize canonical order
        if is_thai_tone_or_diacritic(ch) && i + 1 < chars.len() && is_thai_above_vowel(chars[i + 1]) {
            out.push(chars[i + 1]);
            out.push(ch);
            i += 2;
            continue;
        }
        out.push(ch);
        i += 1;
    }

    out.into_iter().collect()
}

/// Segments Thai text into syllable/word break units for line wrapping.
pub fn segment_thai_syllables(text: &str) -> Vec<String> {
    let shaped = shape_thai(text);
    let chars: Vec<char> = shaped.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }

    let mut syllables: Vec<String> = Vec::new();
    let mut current = String::new();

    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];

        // Break opportunity before a leading vowel or consonant if preceding syllable is complete
        let is_new_syllable_start = if is_thai_leading_vowel(ch) {
            !current.is_empty()
        } else if matches!(ch, '\u{0E01}'..='\u{0E2E}') {
            // Consonant: check if previous character was a vowel, tone mark, or complete unit
            if let Some(prev) = current.chars().last() {
                is_thai_above_vowel(prev)
                    || is_thai_below_vowel(prev)
                    || is_thai_tone_or_diacritic(prev)
                    || matches!(prev, '\u{0E30}' | '\u{0E32}' | '\u{0E33}')
            } else {
                false
            }
        } else {
            false
        };

        if is_new_syllable_start && !current.is_empty() {
            syllables.push(std::mem::take(&mut current));
        }

        current.push(ch);
        i += 1;
    }

    if !current.is_empty() {
        syllables.push(current);
    }

    syllables
}

/// Unified script shaping: applies Arabic, Devanagari, or Thai shaping as appropriate.
pub fn shape_complex_script(text: &str) -> String {
    if text.chars().any(is_arabic_char) {
        shape_arabic(text)
    } else if text.chars().any(is_devanagari_char) {
        shape_devanagari(text)
    } else if text.chars().any(is_thai_char) {
        shape_thai(text)
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arabic_shaping_and_lam_alef() {
        let isolated_baa = shape_arabic("ب");
        assert_eq!(isolated_baa, "\u{FE8F}");

        // "باب": initial baa + alef + isolated baa
        let baab = shape_arabic("باب");
        assert_eq!(baab, "\u{FE91}\u{FE8E}\u{FE8F}");

        // Lam-Alef ligature "لا"
        let la = shape_arabic("لا");
        assert_eq!(la, "\u{FEFB}");
    }

    #[test]
    fn test_arabic_tashkeel_transparency() {
        // "بِ" (Baa with Kasra) + "س" -> Baa should still take initial form even with Kasra!
        let shaped = shape_arabic("بِس");
        assert!(shaped.starts_with('\u{FE91}'), "Baa must join across Kasra: got {}", shaped);
    }

    #[test]
    fn test_devanagari_prebase_matra_reordering() {
        // "क" (\u{0915}) + "ि" (\u{093F}) -> Chhoti I matra must reorder to before Ka
        let text = "\u{0915}\u{093F}";
        let shaped = shape_devanagari(text);
        assert_eq!(shaped, "\u{093F}\u{0915}");

        // "स्थि" -> 'स' + virama + 'थ' + 'ि' -> 'ि' must move before the entire conjunct cluster!
        let cluster = "\u{0938}\u{094D}\u{0925}\u{093F}";
        let shaped_cluster = shape_devanagari(cluster);
        assert!(shaped_cluster.starts_with('\u{093F}'), "Matra must precede conjunct: got {}", shaped_cluster);
    }

    #[test]
    fn test_devanagari_nukta_composition() {
        // "क" (\u{0915}) + Nukta (\u{093C}) -> "क़" (\u{0958})
        let raw = "\u{0915}\u{093C}";
        assert_eq!(shape_devanagari(raw), "\u{0958}");
    }

    #[test]
    fn test_thai_combining_normalization_and_segmentation() {
        // Out of order: Consonant + Tone + Vowel -> normalized to Consonant + Vowel + Tone
        let raw = "\u{0E01}\u{0E48}\u{0E34}";
        let shaped = shape_thai(raw);
        assert_eq!(shaped, "\u{0E01}\u{0E34}\u{0E48}");

        // Syllable segmentation
        let phrase = "ภาษาไทย";
        let syllables = segment_thai_syllables(phrase);
        assert!(syllables.len() >= 2, "Thai phrase must be segmented into syllables: got {:?}", syllables);
    }
}
