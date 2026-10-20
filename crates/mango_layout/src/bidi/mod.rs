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

pub mod brackets;
pub mod class;
pub mod reorder;
pub mod rules;
pub mod stack;

#[cfg(test)]
mod tests;

pub use brackets::{BracketType, bracket_type, mirror_char};
pub use class::{BidiClass, bidi_class};
pub use reorder::{BidiRun, bidi_visual_runs, reorder_bidi_text, resolve_base_direction};
pub use rules::{apply_weak_and_neutral_rules, resolve_bracket_pairs};
pub use stack::{
    DirectionalEntry, MAX_DEPTH, next_even, next_odd, resolve_explicit_levels,
    resolve_isolate_direction,
};
