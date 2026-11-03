use super::brackets::mirror_char;
use super::class::{BidiClass, bidi_class};
use unicode_bidi::{BidiInfo, Level};

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

/// Executes the Unicode Bidirectional Algorithm (UAX#9) on a paragraph of text.
///
/// Returns visually ordered characters ready for left-to-right rasterization.
pub fn reorder_bidi_text(text: &str, is_rtl_base: bool) -> String {
    if text.is_empty() {
        return String::new();
    }

    let default_level = if is_rtl_base {
        Some(Level::rtl())
    } else {
        Some(Level::ltr())
    };

    let bidi_info = BidiInfo::new(text, default_level);
    let mut result = String::with_capacity(text.len());

    for para in &bidi_info.paragraphs {
        let line = para.range.clone();
        let (levels, runs) = bidi_info.visual_runs(para, line);
        for run in runs {
            let run_level = levels[run.start];
            let run_str = &text[run];
            if run_level.is_rtl() {
                for ch in run_str.chars().rev() {
                    let mirrored = mirror_char(ch);
                    if !matches!(
                        mirrored,
                        '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200B}' | '\u{FEFF}'
                    ) {
                        result.push(mirrored);
                    }
                }
            } else {
                for ch in run_str.chars() {
                    if !matches!(
                        ch,
                        '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200B}' | '\u{FEFF}'
                    ) {
                        result.push(ch);
                    }
                }
            }
        }
    }

    result
}

/// Splits text into visual directional runs according to UAX#9 levels.
pub fn bidi_visual_runs(text: &str, is_rtl_base: bool) -> Vec<BidiRun> {
    if text.is_empty() {
        return Vec::new();
    }

    let default_level = if is_rtl_base {
        Some(Level::rtl())
    } else {
        Some(Level::ltr())
    };

    let bidi_info = BidiInfo::new(text, default_level);
    let mut out_runs = Vec::new();

    for para in &bidi_info.paragraphs {
        let line = para.range.clone();
        let (levels, runs) = bidi_info.visual_runs(para, line);
        for run in runs {
            let run_level = levels[run.start];
            let is_rtl = run_level.is_rtl();
            let run_str = &text[run];
            let run_text: String = if is_rtl {
                run_str
                    .chars()
                    .rev()
                    .map(mirror_char)
                    .filter(|&c| !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
                    .collect()
            } else {
                run_str
                    .chars()
                    .filter(|&c| !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
                    .collect()
            };

            if !run_text.is_empty() {
                out_runs.push(BidiRun {
                    text: run_text,
                    level: run_level.number(),
                    is_rtl,
                });
            }
        }
    }

    out_runs
}
