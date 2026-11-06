//! UAX #24 (Unicode Standard Annex #24) Unicode Script Run Segmentation.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnicodeScript {
    Latin,
    Devanagari,
    Bengali,
    Gurmukhi,
    Gujarati,
    Oriya,
    Tamil,
    Telugu,
    Kannada,
    Malayalam,
    Sinhala,
    Arabic,
    Hebrew,
    Thai,
    Lao,
    Armenian,
    Georgian,
    Cjk,
    Common,
    Inherited,
    Other,
}

impl UnicodeScript {
    /// Returns true if the script requires OpenType shaping (HarfBuzz).
    #[inline]
    pub fn is_complex(&self) -> bool {
        matches!(
            self,
            UnicodeScript::Devanagari
                | UnicodeScript::Bengali
                | UnicodeScript::Gurmukhi
                | UnicodeScript::Gujarati
                | UnicodeScript::Oriya
                | UnicodeScript::Tamil
                | UnicodeScript::Telugu
                | UnicodeScript::Kannada
                | UnicodeScript::Malayalam
                | UnicodeScript::Sinhala
                | UnicodeScript::Arabic
                | UnicodeScript::Thai
                | UnicodeScript::Lao
        )
    }
}

/// Classifies a character into a `UnicodeScript`.
pub fn char_unicode_script(ch: char) -> UnicodeScript {
    let u = ch as u32;
    match u {
        0x0000..=0x0040 | 0x005B..=0x0060 | 0x007B..=0x00A9 => {
            if ch.is_whitespace() || ch.is_ascii_punctuation() || ch.is_ascii_digit() {
                UnicodeScript::Common
            } else {
                UnicodeScript::Latin
            }
        }
        0x0041..=0x005A | 0x0061..=0x007A | 0x00C0..=0x024F | 0x1E00..=0x1EFF => {
            UnicodeScript::Latin
        }
        0x0900..=0x097F | 0xA8E0..=0xA8FF => UnicodeScript::Devanagari,
        0x0980..=0x09FF => UnicodeScript::Bengali,
        0x0A00..=0x0A7F => UnicodeScript::Gurmukhi,
        0x0A80..=0x0AFF => UnicodeScript::Gujarati,
        0x0B00..=0x0B7F => UnicodeScript::Oriya,
        0x0B80..=0x0BFF => UnicodeScript::Tamil,
        0x0C00..=0x0C7F => UnicodeScript::Telugu,
        0x0C80..=0x0CFF => UnicodeScript::Kannada,
        0x0D00..=0x0D7F => UnicodeScript::Malayalam,
        0x0D80..=0x0DFF => UnicodeScript::Sinhala,
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => {
            UnicodeScript::Arabic
        }
        0x0590..=0x05FF | 0xFB1D..=0xFB4F => UnicodeScript::Hebrew,
        0x0E00..=0x0E7F => UnicodeScript::Thai,
        0x0E80..=0x0EFF => UnicodeScript::Lao,
        0x0530..=0x058F => UnicodeScript::Armenian,
        0x10A0..=0x10FF => UnicodeScript::Georgian,
        0x3000..=0x303F | 0x2000..=0x206F => UnicodeScript::Common,
        0x4E00..=0x9FFF
        | 0x3400..=0x4DBF
        | 0x3040..=0x30FF
        | 0x31F0..=0x31FF
        | 0xAC00..=0xD7AF
        | 0x1100..=0x11FF
        | 0x3130..=0x318F => UnicodeScript::Cjk,
        0x0300..=0x036F | 0x20D0..=0x20FF => UnicodeScript::Inherited,
        _ => {
            if ch.is_whitespace() || ch.is_ascii_punctuation() {
                UnicodeScript::Common
            } else {
                UnicodeScript::Other
            }
        }
    }
}

/// A contiguous slice of text belonging to a single Unicode script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptRun<'a> {
    pub text: &'a str,
    pub script: UnicodeScript,
    pub representative_char: char,
}

/// Segments text into contiguous script runs according to UAX #24.
/// Common and Inherited characters adopt the active script of the run.
pub fn segment_script_runs<'a>(text: &'a str) -> Vec<ScriptRun<'a>> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut runs = Vec::new();
    let mut current_start = 0;
    let mut current_script = UnicodeScript::Common;
    let mut representative_ch = ' ';

    for (idx, ch) in text.char_indices() {
        let ch_script = char_unicode_script(ch);

        if ch_script == UnicodeScript::Common || ch_script == UnicodeScript::Inherited {
            // Inherit active script
            continue;
        }

        if current_script == UnicodeScript::Common {
            // First typed script encountered in the string or run
            current_script = ch_script;
            representative_ch = ch;
        } else if ch_script != current_script {
            // Script boundary reached: emit preceding run
            if idx > current_start {
                runs.push(ScriptRun {
                    text: &text[current_start..idx],
                    script: current_script,
                    representative_char: representative_ch,
                });
            }
            current_start = idx;
            current_script = ch_script;
            representative_ch = ch;
        }
    }

    if current_start < text.len() {
        let final_script = if current_script == UnicodeScript::Common {
            UnicodeScript::Latin
        } else {
            current_script
        };
        let rep = if representative_ch == ' ' {
            text[current_start..].chars().next().unwrap_or(' ')
        } else {
            representative_ch
        };
        runs.push(ScriptRun {
            text: &text[current_start..],
            script: final_script,
            representative_char: rep,
        });
    }

    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_google_multilingual_offered_in() {
        let text = "Google offered in: हिन्दी বাংলা తెలుగు मराठी தமிழ் ગુજરાતી ಕನ್ನಡ മലയാളം ਪੰਜਾਬੀ";
        let runs = segment_script_runs(text);
        assert!(!runs.is_empty());

        let scripts: Vec<UnicodeScript> = runs.iter().map(|r| r.script).collect();
        assert_eq!(scripts[0], UnicodeScript::Latin);
        assert!(scripts.contains(&UnicodeScript::Devanagari));
        assert!(scripts.contains(&UnicodeScript::Bengali));
        assert!(scripts.contains(&UnicodeScript::Telugu));
        assert!(scripts.contains(&UnicodeScript::Tamil));
        assert!(scripts.contains(&UnicodeScript::Gujarati));
        assert!(scripts.contains(&UnicodeScript::Kannada));
        assert!(scripts.contains(&UnicodeScript::Malayalam));
        assert!(scripts.contains(&UnicodeScript::Gurmukhi));
    }

    #[test]
    fn test_segment_pure_latin_and_pure_indic() {
        let latin_runs = segment_script_runs("Hello World 123!");
        assert_eq!(latin_runs.len(), 1);
        assert_eq!(latin_runs[0].script, UnicodeScript::Latin);

        let hindi_runs = segment_script_runs("नमस्ते दुनिया");
        assert_eq!(hindi_runs.len(), 1);
        assert_eq!(hindi_runs[0].script, UnicodeScript::Devanagari);
    }
}
