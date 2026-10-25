use super::class::BidiClass;

/// Maximum depth for the directional status stack (UAX#9 §3.3.2).
pub const MAX_DEPTH: u8 = 125;

/// An entry on the directional status stack.
#[derive(Debug, Clone, Copy)]
pub struct DirectionalEntry {
    pub level: u8,
    pub override_status: Option<BidiClass>, // None = neutral, Some(L) or Some(R)
    pub isolate_status: bool,
}

/// Computes the least greater odd level.
#[inline]
pub fn next_odd(level: u8) -> u8 {
    (level + 1) | 1
}

/// Computes the least greater even level.
#[inline]
pub fn next_even(level: u8) -> u8 {
    (level + 2) & !1
}

/// Executes Rules X1–X8 to compute embedding levels.
///
/// Returns the resolved levels, resolved types, and a boolean array marking
/// which positions are isolate initiators.
pub fn resolve_explicit_levels(chars: &[char], types: &mut [BidiClass], base_level: u8) -> Vec<u8> {
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
        let current_level = stack.last().map(|e| e.level).unwrap_or(base_level);

        match original_type {
            // X2: RLE
            BidiClass::RLE => {
                let new_level = next_odd(current_level);
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
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
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
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
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
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
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
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
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
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
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
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
                let is_rtl = resolve_isolate_direction(chars, types, i + 1);
                levels[i] = current_level;
                let new_level = if is_rtl {
                    next_odd(current_level)
                } else {
                    next_even(current_level)
                };
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
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
                } else if stack.len() >= 2
                    && !stack.last().map(|e| e.isolate_status).unwrap_or(false)
                {
                    stack.pop();
                }
                levels[i] = stack.last().map(|e| e.level).unwrap_or(base_level);
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
                        if stack.last().map(|e| e.isolate_status).unwrap_or(false) {
                            stack.pop();
                            break;
                        }
                        stack.pop();
                    }
                    valid_isolate_count -= 1;
                }
                levels[i] = stack.last().map(|e| e.level).unwrap_or(base_level);
                types[i] = BidiClass::ON;
            }
            // X6: All other characters
            _ => {
                levels[i] = current_level;
                if let Some(ovr) = stack.last().and_then(|e| e.override_status) {
                    types[i] = ovr;
                }
            }
        }
    }

    levels
}

/// Resolves the direction of text inside an FSI isolate for Rule X5c.
pub fn resolve_isolate_direction(chars: &[char], types: &[BidiClass], start: usize) -> bool {
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
