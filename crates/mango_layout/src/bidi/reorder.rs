use super::brackets::mirror_char;
use super::class::{BidiClass, bidi_class};
use super::rules::apply_weak_and_neutral_rules;
use super::stack::resolve_explicit_levels;

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
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }

    let n = chars.len();
    let base_level: u8 = if is_rtl_base { 1 } else { 0 };

    // Initial classes
    let mut types: Vec<BidiClass> = chars.iter().copied().map(bidi_class).collect();

    // X1–X8: Explicit embedding/override/isolate levels
    let mut levels = resolve_explicit_levels(&chars, &mut types, base_level);

    // Apply W1–W7, N0 (bracket pairs), N1–N2, I1–I2, L1
    apply_weak_and_neutral_rules(&mut types, &mut levels, &chars, base_level, is_rtl_base);

    // --- L4: Mirroring in odd levels ---
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
pub fn bidi_visual_runs(text: &str, is_rtl_base: bool) -> Vec<BidiRun> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }

    let n = chars.len();
    let base_level: u8 = if is_rtl_base { 1 } else { 0 };

    // Run the level computation
    let mut types: Vec<BidiClass> = chars.iter().copied().map(bidi_class).collect();
    let mut levels = resolve_explicit_levels(&chars, &mut types, base_level);

    // Apply W1–W7, N1–N2, I1–I2
    apply_weak_and_neutral_rules(&mut types, &mut levels, &chars, base_level, is_rtl_base);

    // Build runs from the level array
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
