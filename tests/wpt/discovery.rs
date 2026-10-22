//! WPT test discovery and metadata handling.
//!
//! Mirrors the parts of `wptrunner`'s metadata handling that matter for a
//! conformance report:
//! * `META.yml` `skip:` directives (file globs relative to the META.yml's
//!   directory) exclude a test from the run.
//! * `META.yml` `include:` directives restrict a directory to a subset.
//! * `WEB_FEATURES.yml` `features:` / `unsupported:` lists gate tests on
//!   optional platform features.
//!
//! The parser is intentionally a small, dependency-free subset of YAML: the
//! upstream metadata files use a very restricted subset (lists of strings and
//! lists of maps).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Metadata for a directory of WPT tests.
#[derive(Debug, Clone, Default)]
pub struct DirectoryMeta {
    /// Globs (relative to the directory) that must be skipped.
    pub skip: Vec<String>,
    /// Globs (relative to the directory) that are the only tests to run.
    pub include: Vec<String>,
    /// Optional feature names required by the tests in this directory.
    pub required_features: Vec<String>,
    /// Optional feature names that are explicitly unsupported.
    pub unsupported_features: Vec<String>,
}

/// Parses the subset of YAML used by WPT's `META.yml` files.
pub fn parse_meta_yaml(src: &str) -> DirectoryMeta {
    let mut meta = DirectoryMeta::default();
    let mut current_key: Option<String> = None;

    for raw_line in src.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }

        // A new mapping key at indentation 0.
        if !line.starts_with(' ')
            && !line.starts_with('-')
            && let Some((k, v)) = line.split_once(':')
        {
            let key = k.trim().to_ascii_lowercase();
            let value = v.trim();
            current_key = Some(key.clone());
            if value.is_empty() {
                continue;
            }
            if key == "skip" {
                meta.skip.push(unquote(value));
            } else if key == "include" {
                meta.include.push(unquote(value));
            }
            continue;
        }

        // A sequence item.
        let trimmed = line.trim_start();
        if let Some(item) = trimmed.strip_prefix("- ") {
            let item = unquote(item.trim());
            match current_key.as_deref() {
                Some("skip") => meta.skip.push(item),
                Some("include") => meta.include.push(item),
                Some("features") => meta.required_features.push(item),
                Some("unsupported") => meta.unsupported_features.push(item),
                _ => {}
            }
            continue;
        }

        // A nested mapping key such as `features:` inside an item, or
        // `unsupported:` / `features:` lists.
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let value = v.trim();
            current_key = Some(key.clone());
            if value.is_empty() {
                continue;
            }
            match key.as_str() {
                "skip" => meta.skip.push(unquote(value)),
                "include" => meta.include.push(unquote(value)),
                _ => {}
            }
        }
    }

    meta
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let s = s
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .unwrap_or(s);
    let s = s.trim();
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Loads `META.yml` for a directory, if present.
pub fn load_meta(dir: &Path) -> DirectoryMeta {
    let path = dir.join("META.yml");
    match std::fs::read_to_string(path) {
        Ok(src) => parse_meta_yaml(&src),
        Err(_) => DirectoryMeta::default(),
    }
}

/// Loads `WEB_FEATURES.yml` for a directory, if present.
pub fn load_web_features(dir: &Path) -> DirectoryMeta {
    let path = dir.join("WEB_FEATURES.yml");
    match std::fs::read_to_string(path) {
        Ok(src) => parse_meta_yaml(&src),
        Err(_) => DirectoryMeta::default(),
    }
}

/// A very small glob matcher supporting the `*` wildcard used by WPT metadata.
///
/// `*` matches any run of characters except `/`; `**` matches across
/// directories. A bare directory name matches everything below it.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    if pattern == "*" || pattern == "**" {
        return true;
    }

    // A pattern without a wildcard matches the path itself or anything below it.
    if !pattern.contains('*') {
        return path == pattern || path.starts_with(&format!("{pattern}/"));
    }

    glob_segments(pattern, path)
}

fn glob_segments(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').collect();
    let seg: Vec<&str> = path.split('/').collect();
    match_segments(&pat, &seg)
}

fn match_segments(pat: &[&str], seg: &[&str]) -> bool {
    if pat.is_empty() {
        return seg.is_empty();
    }
    if pat[0] == "**" {
        // `**` matches zero or more path segments.
        for i in 0..=seg.len() {
            if match_segments(&pat[1..], &seg[i..]) {
                return true;
            }
        }
        return false;
    }
    if seg.is_empty() {
        return false;
    }
    if !wildcard_match(pat[0], seg[0]) {
        return false;
    }
    match_segments(&pat[1..], &seg[1..])
}

/// Matches a single path segment against a pattern containing `*`.
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }

    let mut cursor = 0usize;
    // First part must match at the beginning.
    if !parts[0].is_empty() {
        if !text.starts_with(parts[0]) {
            return false;
        }
        cursor = parts[0].len();
    }

    // Last part must match at the end.
    let last = parts[parts.len() - 1];
    if !last.is_empty() {
        if !text.ends_with(last) {
            return false;
        }
        let end = text.len() - last.len();
        if end < cursor {
            return false;
        }
        cursor = end;
    }

    // Middle parts must appear in order.
    for part in &parts[1..parts.len() - 1] {
        if part.is_empty() {
            continue;
        }
        match text[cursor..].find(part) {
            Some(idx) => cursor += idx + part.len(),
            None => return false,
        }
    }

    true
}

/// Determines whether a test file (relative to the WPT root) is skipped by any
/// `META.yml` in its ancestor directories.
///
/// `META.yml` skip globs are relative to the directory containing the
/// `META.yml`, matching wptrunner's behaviour.
pub fn is_skipped(root: &Path, rel_path: &str) -> bool {
    let mut dir = root.to_path_buf();
    let mut segments: Vec<&str> = rel_path.split('/').collect();
    let file_name = segments.pop().unwrap_or_default();

    loop {
        let meta = load_meta(&dir);
        if !meta.skip.is_empty() {
            let rel_to_dir = match dir.strip_prefix(root) {
                Ok(r) => r.to_string_lossy().replace('\\', "/"),
                Err(_) => String::new(),
            };
            let rel_to_dir = rel_to_dir.trim_end_matches('/').to_string();
            let candidate = if rel_to_dir.is_empty() {
                file_name.to_string()
            } else {
                format!("{rel_to_dir}/{file_name}")
            };
            for pattern in &meta.skip {
                if glob_match(pattern, &candidate) {
                    return true;
                }
            }
        }
        if dir == root || !dir.starts_with(root) {
            break;
        }
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => break,
        }
    }
    false
}

/// Collects the set of directories (relative to `root`) that contain a
/// `META.yml`, used to report the effective configuration.
pub fn meta_directories(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.join("META.yml").exists() {
                    out.push(path.clone());
                }
                walk(&path, out);
            }
        }
    }
    walk(root, &mut out);
    out
}

/// Returns the set of distinct top-level categories present under `root`.
///
/// Categories are the first path segment of every test file, e.g. `css`,
/// `dom`, `html`, `webapi`.
pub fn categories(root: &Path) -> HashSet<String> {
    let mut out = HashSet::new();
    for rel in super::harness::discover_tests(root, root) {
        if let Some(first) = rel.split('/').next() {
            out.insert(first.to_string());
        }
    }
    out
}
