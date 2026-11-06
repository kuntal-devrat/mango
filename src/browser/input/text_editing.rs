//! Text editing and cursor navigation powered by `unicode-segmentation`.
//!
//! Provides grapheme cluster awareness (preventing splitting multi-byte emoji,
//! skin tones, and combining characters) and word boundary detection for
//! Ctrl+Arrow / Ctrl+Backspace / Ctrl+Delete navigation.

use unicode_segmentation::UnicodeSegmentation;

/// Finds the byte offset of the previous grapheme cluster boundary before `cursor_pos`.
pub fn prev_grapheme_offset(s: &str, cursor_pos: usize) -> usize {
    let pos = cursor_pos.min(s.len());
    s.grapheme_indices(true)
        .take_while(|&(idx, _)| idx < pos)
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

/// Finds the byte offset of the next grapheme cluster boundary after `cursor_pos`.
pub fn next_grapheme_offset(s: &str, cursor_pos: usize) -> usize {
    let pos = cursor_pos.min(s.len());
    s.grapheme_indices(true)
        .map(|(idx, g)| idx + g.len())
        .find(|&idx| idx > pos)
        .unwrap_or(s.len())
}

/// Finds the byte offset of the previous word boundary before `cursor_pos`
/// (for Ctrl+Left and Ctrl+Backspace).
pub fn prev_word_boundary(s: &str, cursor_pos: usize) -> usize {
    let pos = cursor_pos.min(s.len());
    if pos == 0 {
        return 0;
    }

    let bounds: Vec<(usize, &str)> = s
        .split_word_bound_indices()
        .filter(|&(idx, _)| idx < pos)
        .collect();

    // If cursor is preceded by spaces/punctuation, skip them to jump to the word before
    let mut i = bounds.len();
    while i > 0 {
        let (idx, word) = bounds[i - 1];
        if word.chars().any(|c| c.is_alphanumeric()) {
            return idx;
        }
        i -= 1;
    }
    bounds.first().map(|&(idx, _)| idx).unwrap_or(0)
}

/// Finds the byte offset of the next word boundary after `cursor_pos`
/// (for Ctrl+Right and Ctrl+Delete).
pub fn next_word_boundary(s: &str, cursor_pos: usize) -> usize {
    let pos = cursor_pos.min(s.len());
    if pos >= s.len() {
        return s.len();
    }

    let mut past_cursor = false;
    for (idx, word) in s.split_word_bound_indices() {
        if idx > pos {
            if past_cursor && word.chars().any(|c| c.is_alphanumeric()) {
                return idx;
            }
            if word.chars().any(|c| c.is_alphanumeric()) {
                past_cursor = true;
            }
        }
    }
    s.len()
}

/// Deletes the grapheme cluster immediately before `cursor_pos` in `text`.
/// Returns the updated cursor byte position.
pub fn backspace_grapheme(text: &mut String, cursor_pos: usize) -> usize {
    if cursor_pos == 0 || text.is_empty() {
        return 0;
    }
    let prev = prev_grapheme_offset(text, cursor_pos);
    let safe_pos = cursor_pos.min(text.len());
    text.drain(prev..safe_pos);
    prev
}

/// Deletes the grapheme cluster immediately after `cursor_pos` in `text`.
pub fn delete_grapheme(text: &mut String, cursor_pos: usize) {
    if cursor_pos >= text.len() || text.is_empty() {
        return;
    }
    let next = next_grapheme_offset(text, cursor_pos);
    let safe_pos = cursor_pos.min(text.len());
    text.drain(safe_pos..next);
}

/// Deletes the word immediately before `cursor_pos` in `text` (Ctrl+Backspace).
/// Returns the updated cursor byte position.
pub fn backspace_word(text: &mut String, cursor_pos: usize) -> usize {
    if cursor_pos == 0 || text.is_empty() {
        return 0;
    }
    let prev = prev_word_boundary(text, cursor_pos);
    let safe_pos = cursor_pos.min(text.len());
    text.drain(prev..safe_pos);
    prev
}

/// Deletes the word immediately after `cursor_pos` in `text` (Ctrl+Delete).
pub fn delete_word(text: &mut String, cursor_pos: usize) {
    if cursor_pos >= text.len() || text.is_empty() {
        return;
    }
    let next = next_word_boundary(text, cursor_pos);
    let safe_pos = cursor_pos.min(text.len());
    text.drain(safe_pos..next);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grapheme_backspace_and_navigation() {
        let mut s = "Hello 🥭 world".to_string();
        // Emoji '🥭' is at byte offset 6..10 (4 bytes)
        let pos_after_mango = 10;
        let prev = prev_grapheme_offset(&s, pos_after_mango);
        assert_eq!(prev, 6);

        // Deleting grapheme at pos 10 removes the whole mango emoji in one stroke
        let new_pos = backspace_grapheme(&mut s, pos_after_mango);
        assert_eq!(new_pos, 6);
        assert_eq!(s, "Hello  world");
    }

    #[test]
    fn test_word_boundaries_ctrl_navigation() {
        let s = "https://duckduckgo.com/?q=rust lang";
        let end = s.len();
        let prev1 = prev_word_boundary(s, end);
        assert_eq!(&s[prev1..], "lang");

        let prev2 = prev_word_boundary(s, prev1);
        assert_eq!(&s[prev2..prev1], "rust ");

        let mut text = "one two three".to_string();
        let len = text.len();
        let p = backspace_word(&mut text, len);
        assert_eq!(text, "one two ");
        assert_eq!(p, 8);
    }
}
