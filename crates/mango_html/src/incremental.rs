//! # Incremental HTML Parser & Preload Scanner (Phase 3.1)
//!
//! Allows feeding HTML bytes or text in streaming chunks as they arrive over the network,
//! discovering speculative preloads (<link>, <script>, <img>) before parsing completes,
//! and constructing the final Document.

use crate::dom::Document;
use crate::tree_builder::parse_html;

/// Kinds of speculative preload hints discovered during streaming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreloadHintKind {
    Style,
    Script,
    Image,
    Font,
    Preconnect,
    DnsPrefetch,
}

/// A speculative preload hint discovered during tokenization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreloadHint {
    pub url: String,
    pub kind: PreloadHintKind,
    pub as_attr: Option<String>,
}

/// Streaming HTML parser that extracts preload hints on-the-fly and builds the DOM.
#[derive(Debug, Default)]
pub struct IncrementalParser {
    /// Decoded complete UTF-8 text accumulated so far.
    buffer: String,
    /// 0-3 byte incomplete UTF-8 tail waiting for more data.
    pending: Vec<u8>,
    scanned_up_to: usize,
    discovered_hints: Vec<PreloadHint>,
}

impl IncrementalParser {
    /// Creates a new, empty incremental parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds a chunk of UTF-8 text into the parser.
    ///
    /// Returns any newly discovered speculative preload hints in this chunk.
    pub fn feed_str(&mut self, chunk: &str) -> Vec<PreloadHint> {
        self.push_bytes(chunk.as_bytes());
        self.scan_new_hints()
    }

    /// Feeds a chunk of raw bytes into the parser.
    ///
    /// Only the complete UTF-8 prefix is decoded; a 0-3 byte incomplete
    /// tail is retained for the next chunk (no lossy split of multi-byte chars).
    /// Returns any newly discovered speculative preload hints in this chunk.
    pub fn feed_bytes(&mut self, chunk: &[u8]) -> Vec<PreloadHint> {
        self.push_bytes(chunk);
        self.scan_new_hints()
    }

    fn push_bytes(&mut self, chunk: &[u8]) {
        if chunk.is_empty() && self.pending.is_empty() {
            return;
        }
        // Combine pending tail + new chunk.
        let mut combined: Vec<u8> = std::mem::take(&mut self.pending);
        combined.extend_from_slice(chunk);
        let tail = incomplete_tail_len(&combined);
        let complete_len = combined.len() - tail;
        let (complete, rest) = combined.split_at(complete_len);
        // Decode complete prefix lossy (handles invalid middle bytes with U+FFFD).
        let s = String::from_utf8_lossy(complete);
        self.buffer.push_str(&s);
        self.pending = rest.to_vec();
    }

    /// Scans newly arrived text up to the last complete HTML tag for preload hints.
    fn scan_new_hints(&mut self) -> Vec<PreloadHint> {
        let mut new_hints = Vec::new();

        loop {
            let total_len = self.buffer.len();
            if self.scanned_up_to >= total_len {
                break;
            }
            // scanned_up_to is always on a char boundary (we only advance past ASCII).
            let slice = &self.buffer[self.scanned_up_to..];
            let Some(tag_open) = slice.find('<') else {
                // No tag start; advance to end to avoid rescanning text.
                self.scanned_up_to = total_len;
                break;
            };

            let tag_abs_start = self.scanned_up_to + tag_open;
            // '<' is 1 byte so +1 is a boundary.
            let after_open = &self.buffer[tag_abs_start + 1..];

            // If it's a comment `<!-- ... -->` (or `--!>` close), skip it.
            if after_open.starts_with("!--") {
                let end1 = after_open.find("-->");
                let end2 = after_open.find("--!>");
                let end = match (end1, end2) {
                    (Some(a), Some(b)) => {
                        if a < b {
                            Some((a, 3))
                        } else {
                            Some((b, 4))
                        }
                    }
                    (Some(a), None) => Some((a, 3)),
                    (None, Some(b)) => Some((b, 4)),
                    (None, None) => None,
                };
                if let Some((pos, len)) = end {
                    self.scanned_up_to = tag_abs_start + 1 + pos + len;
                    continue;
                } else {
                    // Incomplete comment across chunk boundary, wait for more data
                    break;
                }
            }

            // Find closing `>` with quote-aware state machine
            // (tracks in_single/in_double so `>` inside attrs doesn't end the tag).
            let bytes = after_open.as_bytes();
            let mut in_single = false;
            let mut in_double = false;
            let mut close_pos: Option<usize> = None;
            for (i, &b) in bytes.iter().enumerate() {
                if b == b'\'' && !in_double {
                    in_single = !in_single;
                } else if b == b'"' && !in_single {
                    in_double = !in_double;
                } else if b == b'>' && !in_single && !in_double {
                    close_pos = Some(i);
                    break;
                }
            }
            let Some(tag_close) = close_pos else {
                // Incomplete tag across chunk boundary, wait for next chunk
                break;
            };

            let tag_content = after_open[..tag_close].trim();
            self.scanned_up_to = tag_abs_start + 1 + tag_close + 1;

            for hint in parse_tag_for_preload(tag_content) {
                if !self.discovered_hints.contains(&hint) {
                    self.discovered_hints.push(hint.clone());
                    new_hints.push(hint);
                }
            }
        }

        new_hints
    }

    /// Returns all hints discovered so far.
    pub fn hints(&self) -> &[PreloadHint] {
        &self.discovered_hints
    }

    /// Returns the accumulated HTML text.
    pub fn accumulated_html(&self) -> &str {
        &self.buffer
    }

    /// Constructs and returns a live snapshot of the [`Document`] tree parsed so far.
    ///
    /// Enables early paint passes (FCP - First Contentful Paint) while the document
    /// is still downloading over the network.
    pub fn current_document(&self) -> Document {
        // Include any pending tail decoded lossy for snapshot purposes only
        // (do not mutate parser state).
        if self.pending.is_empty() {
            parse_html(&self.buffer)
        } else {
            let mut tmp = self.buffer.clone();
            tmp.push_str(&String::from_utf8_lossy(&self.pending));
            parse_html(&tmp)
        }
    }

    /// Finalizes parsing and returns the complete [`Document`].
    pub fn finish(mut self) -> Document {
        if !self.pending.is_empty() {
            let tail = std::mem::take(&mut self.pending);
            self.buffer.push_str(&String::from_utf8_lossy(&tail));
        }
        parse_html(&self.buffer)
    }
}

/// Returns the length (0-3) of an incomplete UTF-8 tail at the end of `data`.
///
/// If the data ends on a char boundary, returns 0. Otherwise returns the number
/// of trailing bytes that form a truncated multi-byte sequence.
fn incomplete_tail_len(data: &[u8]) -> usize {
    if data.is_empty() {
        return 0;
    }
    // Fast path: valid UTF-8 entirely.
    if std::str::from_utf8(data).is_ok() {
        return 0;
    }
    // Walk back over continuation bytes (10xxxxxx), up to 3.
    let mut cont = 0usize;
    for &b in data.iter().rev().take(4) {
        if b & 0xC0 == 0x80 {
            cont += 1;
        } else {
            // b is a leading byte; determine expected length.
            let expected = if b & 0x80 == 0 {
                1
            } else if b & 0xE0 == 0xC0 {
                2
            } else if b & 0xF0 == 0xE0 {
                3
            } else if b & 0xF8 == 0xF0 {
                4
            } else {
                // Invalid leading byte: not an incomplete tail, treat as complete
                // (lossy decode will replace it).
                return 0;
            };
            let available = cont + 1;
            if available < expected {
                return available;
            } else {
                return 0;
            }
        }
    }
    // All last 4 bytes are continuations (invalid); not incomplete.
    0
}

fn rel_token_contains(rel: &str, token: &str) -> bool {
    rel.split_ascii_whitespace()
        .any(|t| t.eq_ignore_ascii_case(token))
}

/// Helper: parses a tag's content (e.g. `link rel="stylesheet" href="style.css"`) for preload hints.
fn parse_tag_for_preload(tag_content: &str) -> Vec<PreloadHint> {
    let name_end = tag_content
        .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
        .unwrap_or(tag_content.len());
    let tag_name = &tag_content[..name_end];
    let rest = &tag_content[name_end..];

    let mut out = Vec::new();
    let push_unique = |out: &mut Vec<PreloadHint>, hint: PreloadHint| {
        if !out.iter().any(|h| h == &hint) {
            out.push(hint);
        }
    };

    if tag_name.eq_ignore_ascii_case("link") {
        let rel = find_attr(rest, "rel").unwrap_or_default();
        let href = find_attr(rest, "href").map(|s| s.trim().to_string()).unwrap_or_default();
        let imagesrcset = find_attr(rest, "imagesrcset");

        if rel_token_contains(&rel, "stylesheet") && !href.is_empty() {
            push_unique(
                &mut out,
                PreloadHint {
                    url: href.clone(),
                    kind: PreloadHintKind::Style,
                    as_attr: None,
                },
            );
        }

        if rel_token_contains(&rel, "preload") {
            let as_attr = find_attr(rest, "as");
            let kind = match as_attr.as_deref().unwrap_or("").to_ascii_lowercase().as_str() {
                "style" => PreloadHintKind::Style,
                "script" => PreloadHintKind::Script,
                "font" => PreloadHintKind::Font,
                "image" => PreloadHintKind::Image,
                _ => PreloadHintKind::Style,
            };
            if !href.is_empty() {
                push_unique(
                    &mut out,
                    PreloadHint {
                        url: href.clone(),
                        kind: kind.clone(),
                        as_attr: as_attr.clone(),
                    },
                );
            }
            // imagesrcset on <link rel=preload as=image>
            if let Some(iss) = imagesrcset {
                for url in parse_srcset(&iss) {
                    if url.is_empty() || url.starts_with("data:") {
                        continue;
                    }
                    push_unique(
                        &mut out,
                        PreloadHint {
                            url,
                            kind: PreloadHintKind::Image,
                            as_attr: as_attr.clone(),
                        },
                    );
                }
            }
            // srcset on link? (rare) handle as well
            if let Some(ss) = find_attr(rest, "srcset") {
                for url in parse_srcset(&ss) {
                    if url.is_empty() || url.starts_with("data:") {
                        continue;
                    }
                    push_unique(
                        &mut out,
                        PreloadHint {
                            url,
                            kind: PreloadHintKind::Image,
                            as_attr: as_attr.clone(),
                        },
                    );
                }
            }
        }

        if rel_token_contains(&rel, "preconnect") && !href.is_empty() {
            push_unique(
                &mut out,
                PreloadHint {
                    url: href.clone(),
                    kind: PreloadHintKind::Preconnect,
                    as_attr: None,
                },
            );
        }

        if rel_token_contains(&rel, "dns-prefetch") && !href.is_empty() {
            push_unique(
                &mut out,
                PreloadHint {
                    url: href.clone(),
                    kind: PreloadHintKind::DnsPrefetch,
                    as_attr: None,
                },
            );
        }
    } else if tag_name.eq_ignore_ascii_case("script") {
        if let Some(src) = find_attr(rest, "src") {
            let trimmed = src.trim().to_string();
            if !trimmed.is_empty() {
                push_unique(
                    &mut out,
                    PreloadHint {
                        url: trimmed,
                        kind: PreloadHintKind::Script,
                        as_attr: None,
                    },
                );
            }
        }
    } else if tag_name.eq_ignore_ascii_case("img") {
        if let Some(src) = find_attr(rest, "src") {
            let trimmed = src.trim().to_string();
            if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                push_unique(
                    &mut out,
                    PreloadHint {
                        url: trimmed,
                        kind: PreloadHintKind::Image,
                        as_attr: None,
                    },
                );
            }
        }
        // srcset / imagesrcset support
        let srcset = find_attr(rest, "srcset");
        let imagesrcset = find_attr(rest, "imagesrcset");
        if let Some(ss) = srcset {
            for url in parse_srcset(&ss) {
                if url.is_empty() || url.starts_with("data:") {
                    continue;
                }
                push_unique(
                    &mut out,
                    PreloadHint {
                        url,
                        kind: PreloadHintKind::Image,
                        as_attr: None,
                    },
                );
            }
        }
        if let Some(iss) = imagesrcset {
            for url in parse_srcset(&iss) {
                if url.is_empty() || url.starts_with("data:") {
                    continue;
                }
                push_unique(
                    &mut out,
                    PreloadHint {
                        url,
                        kind: PreloadHintKind::Image,
                        as_attr: None,
                    },
                );
            }
        }
    } else if tag_name.eq_ignore_ascii_case("source") {
        if let Some(src) = find_attr(rest, "src") {
            let trimmed = src.trim().to_string();
            if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                push_unique(
                    &mut out,
                    PreloadHint {
                        url: trimmed,
                        kind: PreloadHintKind::Image,
                        as_attr: None,
                    },
                );
            }
        }
        if let Some(ss) = find_attr(rest, "srcset") {
            for url in parse_srcset(&ss) {
                if url.is_empty() || url.starts_with("data:") {
                    continue;
                }
                push_unique(
                    &mut out,
                    PreloadHint {
                        url,
                        kind: PreloadHintKind::Image,
                        as_attr: None,
                    },
                );
            }
        }
        if let Some(iss) = find_attr(rest, "imagesrcset") {
            for url in parse_srcset(&iss) {
                if url.is_empty() || url.starts_with("data:") {
                    continue;
                }
                push_unique(
                    &mut out,
                    PreloadHint {
                        url,
                        kind: PreloadHintKind::Image,
                        as_attr: None,
                    },
                );
            }
        }
    } else if tag_name.eq_ignore_ascii_case("video") {
        if let Some(src) = find_attr(rest, "src") {
            let trimmed = src.trim().to_string();
            if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                push_unique(
                    &mut out,
                    PreloadHint {
                        url: trimmed,
                        kind: PreloadHintKind::Image,
                        as_attr: None,
                    },
                );
            }
        }
        if let Some(poster) = find_attr(rest, "poster") {
            let trimmed = poster.trim().to_string();
            if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                push_unique(
                    &mut out,
                    PreloadHint {
                        url: trimmed,
                        kind: PreloadHintKind::Image,
                        as_attr: None,
                    },
                );
            }
        }
        if let Some(ss) = find_attr(rest, "srcset") {
            for url in parse_srcset(&ss) {
                if url.is_empty() || url.starts_with("data:") {
                    continue;
                }
                push_unique(
                    &mut out,
                    PreloadHint {
                        url,
                        kind: PreloadHintKind::Image,
                        as_attr: None,
                    },
                );
            }
        }
    }

    out
}

/// Parses a srcset value into candidate URLs (first token of each comma entry).
fn parse_srcset(srcset: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for candidate in srcset.split(',') {
        let c = candidate.trim();
        if c.is_empty() {
            continue;
        }
        // URL is up to first ASCII whitespace (descriptor follows).
        let end = c
            .find(|ch: char| ch.is_ascii_whitespace())
            .unwrap_or(c.len());
        let url = c[..end].trim();
        if !url.is_empty() {
            urls.push(url.to_string());
        }
    }
    urls
}

/// Finds a single attribute value by name (case-insensitive) without building
/// a HashMap for the whole tag. Returns `None` when absent.
fn find_attr(s: &str, wanted: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        // Skip ASCII whitespace only.
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= len {
            break;
        }
        if bytes[i] == b'/' || bytes[i] == b'>' {
            i += 1;
            continue;
        }
        let name_start = i;
        while i < len
            && bytes[i] != b'='
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'/'
            && bytes[i] != b'>'
        {
            i += 1;
        }
        let name = &s[name_start..i];
        if name.is_empty() {
            i += 1;
            continue;
        }
        let is_wanted = name.eq_ignore_ascii_case(wanted);
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < len && bytes[i] == b'=' {
            i += 1;
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= len {
                if is_wanted {
                    return Some(String::new());
                }
                break;
            }
            let quote = bytes[i];
            if quote == b'"' || quote == b'\'' {
                i += 1;
                let val_start = i;
                while i < len && bytes[i] != quote {
                    i += 1;
                }
                if is_wanted {
                    return Some(s[val_start..i].to_string());
                }
                if i < len {
                    i += 1;
                }
            } else {
                let val_start = i;
                while i < len && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                    i += 1;
                }
                if is_wanted {
                    return Some(s[val_start..i].to_string());
                }
            }
        } else if is_wanted {
            // Boolean attribute.
            return Some(String::new());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incremental_parser_chunk_boundaries() {
        let mut parser = IncrementalParser::new();

        // Feed chunk 1: partial tag across boundary
        let hints1 = parser.feed_str("<!DOCTYPE html><html><head><link rel=\"style");
        assert!(hints1.is_empty(), "Partial tag should yield no hints yet");

        // Feed chunk 2: complete the tag and add script
        let hints2 = parser.feed_str("sheet\" href=\"main.css\"><script src=\"app.js\"></script>");
        assert_eq!(hints2.len(), 2, "Should find style and script hints");
        assert_eq!(hints2[0].url, "main.css");
        assert_eq!(hints2[0].kind, PreloadHintKind::Style);
        assert_eq!(hints2[1].url, "app.js");
        assert_eq!(hints2[1].kind, PreloadHintKind::Script);

        // Feed chunk 3: body with image
        let hints3 = parser.feed_str("</head><body><img src=\"hero.png\"><h1>Hello</h1></body></html>");
        assert_eq!(hints3.len(), 1);
        assert_eq!(hints3[0].url, "hero.png");
        assert_eq!(hints3[0].kind, PreloadHintKind::Image);

        // Finish parsing
        let doc = parser.finish();
        let h1_id = doc.find_element_by_tag(doc.root(), "h1").expect("h1 element found");
        assert_eq!(doc.text_content(h1_id), "Hello");
    }

    #[test]
    fn test_preload_hints_deduplication() {
        let mut parser = IncrementalParser::new();
        let hints1 = parser.feed_str("<link rel=\"stylesheet\" href=\"style.css\">");
        assert_eq!(hints1.len(), 1);

        let hints2 = parser.feed_str("<link rel=\"stylesheet\" href=\"style.css\">");
        assert_eq!(hints2.len(), 0, "Duplicate preload should be deduplicated");
    }

    #[test]
    fn test_incremental_current_document_progressive_fcp() {
        let mut parser = IncrementalParser::new();
        parser.feed_str("<!DOCTYPE html><html><head><title>Test Page</title></head><body><p>Chunk 1</p>");
        let doc1 = parser.current_document();
        let p1 = doc1.find_element_by_tag(doc1.root(), "p").expect("p element present in chunk 1");
        assert_eq!(doc1.text_content(p1), "Chunk 1");

        parser.feed_str("<div>Chunk 2</div></body></html>");
        let doc2 = parser.finish();
        let div = doc2.find_element_by_tag(doc2.root(), "div").expect("div element present in final doc");
        assert_eq!(doc2.text_content(div), "Chunk 2");
    }
}
