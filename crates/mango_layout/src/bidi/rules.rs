use super::brackets::{BracketType, bracket_type};
use super::class::{BidiClass, bidi_class};

/// Rule N0: Resolves paired brackets based on the directionality of enclosed content.
///
/// Implements UAX#9 BD16 (bracket pair detection using a stack, max depth 63)
/// and Rule N0 (resolving pair types using embedding direction and inner/context strong types).
pub fn resolve_bracket_pairs(
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
        let embedding_dir = if embedding_level.is_multiple_of(2) {
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

            let context_dir = prev_strong.unwrap_or(if base_level.is_multiple_of(2) {
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
pub fn apply_weak_and_neutral_rules(
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
    let mut last_strong_l = base_level.is_multiple_of(2);
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
        if levels[i].is_multiple_of(2) {
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
            if idx < n {
                bidi_class(chars[idx])
            } else {
                BidiClass::BN
            }
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
                    BidiClass::WS
                        | BidiClass::LRI
                        | BidiClass::RLI
                        | BidiClass::FSI
                        | BidiClass::PDI
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
