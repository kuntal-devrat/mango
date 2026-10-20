//! Key selector extraction for O(1) rule index candidate bucketing.

use crate::cascade::types::KeySelector;

/// Extracts subject candidate key selectors from `:is(...)` or `:where(...)` pseudo-classes.
/// Returns `Some(keys)` if every branch has a definite subject key, or `None` if any branch is universal.
pub fn extract_subject_keys_from_is_where(pseudo_str: &str) -> Option<Vec<KeySelector>> {
    let inner = if let Some(stripped) = pseudo_str.strip_prefix("is(") {
        stripped.strip_suffix(')')?
    } else {
        let stripped = pseudo_str.strip_prefix("where(")?;
        stripped.strip_suffix(')')?
    };

    let mut branches = Vec::new();
    let mut depth = 0;
    let mut current = String::new();
    for ch in inner.chars() {
        match ch {
            '(' | '[' => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' => {
                if depth > 0 {
                    depth -= 1;
                }
                current.push(ch);
            }
            ',' if depth == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    branches.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        branches.push(trimmed.to_string());
    }

    if branches.is_empty() {
        return None;
    }

    let mut all_keys = Vec::new();
    for branch in branches {
        let mut b_depth = 0;
        let mut last_comb_idx = 0;
        for (i, ch) in branch.char_indices() {
            match ch {
                '(' | '[' => b_depth += 1,
                ')' | ']' => {
                    if b_depth > 0 {
                        b_depth -= 1;
                    }
                }
                ' ' | '>' | '+' | '~' if b_depth == 0 => {
                    last_comb_idx = i + ch.len_utf8();
                }
                _ => {}
            }
        }
        let subject = branch[last_comb_idx..].trim();
        if subject.is_empty() {
            return None;
        }

        let mut key = None;
        if subject.starts_with(":root") {
            key = Some(KeySelector::Root);
        } else {
            let mut tag_end = 0;
            for (idx, c) in subject.char_indices() {
                if c == '.' || c == '#' || c == '[' || c == ':' {
                    tag_end = idx;
                    break;
                }
                tag_end = idx + c.len_utf8();
            }
            let tag_name = subject[..tag_end].trim();
            if !tag_name.is_empty() && tag_name != "*" {
                key = Some(KeySelector::Tag(tag_name.to_ascii_lowercase()));
            }

            let rem = &subject[tag_end..];
            let bytes = rem.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'.' {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len()
                        && (bytes[end].is_ascii_alphanumeric()
                            || bytes[end] == b'-'
                            || bytes[end] == b'_')
                    {
                        end += 1;
                    }
                    if end > start {
                        let cls = &rem[start..end];
                        key = Some(KeySelector::Class(cls.to_string()));
                        break;
                    }
                    i = end;
                } else if bytes[i] == b'#' {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len()
                        && (bytes[end].is_ascii_alphanumeric()
                            || bytes[end] == b'-'
                            || bytes[end] == b'_')
                    {
                        end += 1;
                    }
                    if end > start {
                        let id = &rem[start..end];
                        key = Some(KeySelector::Id(id.to_string()));
                        break;
                    }
                    i = end;
                } else if bytes[i] == b'[' {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len()
                        && (bytes[end].is_ascii_alphanumeric()
                            || bytes[end] == b'-'
                            || bytes[end] == b'_')
                    {
                        end += 1;
                    }
                    if end > start {
                        let attr = &rem[start..end];
                        key = Some(KeySelector::Attr(attr.to_ascii_lowercase()));
                        break;
                    }
                    i += 1;
                } else {
                    i += 1;
                }
            }
        }

        {
            let k = key?;
            all_keys.push(k);
        }
    }

    if all_keys.is_empty() {
        None
    } else {
        Some(all_keys)
    }
}
