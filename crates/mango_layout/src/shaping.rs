//! Complex Script Shaping Engine re-exported from `mango_render::font::shaping`.

pub use mango_render::font::shaping::*;

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
    fn test_arabic_tatweel_joining() {
        // "بـاب": Baa + Tatweel + Alef + Baa -> Baa should take initial form, Tatweel passes through, Alef takes final form
        let text = "\u{0628}\u{0640}\u{0627}\u{0628}";
        let shaped = shape_arabic(text);
        assert_eq!(shaped, "\u{FE91}\u{0640}\u{FE8E}\u{FE8F}");
    }

    #[test]
    fn test_mixed_complex_script_shaping() {
        // String containing both Arabic "باب" and Devanagari "कि"
        let mixed = "باب क\u{093F}";
        let shaped = shape_complex_script(mixed);
        assert_eq!(shaped, "\u{FE91}\u{FE8E}\u{FE8F} \u{093F}\u{0915}");
    }

    #[test]
    fn test_arabic_tashkeel_transparency() {
        // "بِ" (Baa with Kasra) + "س" -> Baa should still take initial form even with Kasra!
        let shaped = shape_arabic("بِس");
        assert!(
            shaped.starts_with('\u{FE91}'),
            "Baa must join across Kasra: got {}",
            shaped
        );
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
        assert!(
            shaped_cluster.starts_with('\u{093F}'),
            "Matra must precede conjunct: got {}",
            shaped_cluster
        );
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
        assert!(
            syllables.len() >= 2,
            "Thai phrase must be segmented into syllables: got {:?}",
            syllables
        );
    }
}
