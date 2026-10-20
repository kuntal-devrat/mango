//! HTML Preload Scanner, DNS-Prefetch, Preconnect & Priority Hints (PRD 10.3).

use crate::url::Url;

// ─────────────────────────────────────────────────────────────────────────────
// HTML Preload Scanner, DNS-Prefetch, Preconnect & Priority Hints (PRD 10.3)
// ─────────────────────────────────────────────────────────────────────────────

/// Preload resource kind specified in `<link rel="preload" as="...">`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreloadKind {
    Style,
    Script,
    Image,
    Font,
    Fetch,
    Other(String),
}

impl PreloadKind {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(val: &str) -> Self {
        match val.trim().to_ascii_lowercase().as_str() {
            "style" | "stylesheet" => PreloadKind::Style,
            "script" => PreloadKind::Script,
            "image" => PreloadKind::Image,
            "font" => PreloadKind::Font,
            "fetch" => PreloadKind::Fetch,
            other => PreloadKind::Other(other.to_string()),
        }
    }
}

/// Priority hint from HTML5 `fetchpriority` attribute (`high`, `low`, `auto`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FetchPriority {
    High,
    #[default]
    Auto,
    Low,
}

impl FetchPriority {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(val: &str) -> Self {
        match val.trim().to_ascii_lowercase().as_str() {
            "high" => FetchPriority::High,
            "low" => FetchPriority::Low,
            _ => FetchPriority::Auto,
        }
    }
}

/// A discovered resource to be preloaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreloadItem {
    pub url: String,
    pub kind: PreloadKind,
    pub priority: FetchPriority,
}

/// Result of scanning an HTML document for preloads, preconnects, and DNS prefetches.
#[derive(Debug, Clone, Default)]
pub struct PreloadScanResult {
    pub preloads: Vec<PreloadItem>,
    pub dns_prefetch_hosts: Vec<String>,
    pub preconnect_urls: Vec<String>,
}

impl PreloadScanResult {
    /// Sorts preloads by priority: High first, then Auto, then Low.
    pub fn sort_by_priority(&mut self) {
        self.preloads.sort_by(|a, b| {
            let rank = |p: FetchPriority| match p {
                FetchPriority::High => 0,
                FetchPriority::Auto => 1,
                FetchPriority::Low => 2,
            };
            rank(a.priority).cmp(&rank(b.priority))
        });
    }
}

/// Fast HTML preload scanner for early resource discovery (PRD 10.3).
pub fn scan_html_for_preloads(html: &str, base_url: &Url) -> PreloadScanResult {
    let mut result = PreloadScanResult::default();

    let mut cursor = 0;
    let len = html.len();

    while cursor < len {
        let Some(tag_start) = html[cursor..].find('<') else {
            break;
        };
        cursor += tag_start + 1;
        if cursor >= len {
            break;
        }

        // Skip comments <!-- ... -->
        if html[cursor..].starts_with("!--") {
            if let Some(comment_end) = html[cursor..].find("-->") {
                cursor += comment_end + 3;
            } else {
                break;
            }
            continue;
        }

        // Find tag end '>'
        let Some(tag_end) = html[cursor..].find('>') else {
            break;
        };
        let tag_slice = &html[cursor..cursor + tag_end];
        cursor += tag_end + 1;

        let trimmed_tag = tag_slice.trim_start();
        let name_end = trimmed_tag
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .unwrap_or(trimmed_tag.len());
        let tag_name = &trimmed_tag[..name_end];

        if tag_name.eq_ignore_ascii_case("link") {
            let attrs = parse_tag_attributes(&trimmed_tag[name_end..]);
            if let Some(rel) = attrs.get("rel") {
                let rel_lower = rel.to_ascii_lowercase();
                if rel_lower.contains("preload") {
                    if let Some(href) = attrs.get("href")
                        && let Ok(resolved) = base_url.resolve(href)
                    {
                        let kind = attrs
                            .get("as")
                            .map(|s| PreloadKind::from_str(s))
                            .unwrap_or(PreloadKind::Fetch);
                        let priority = attrs
                            .get("fetchpriority")
                            .map(|s| FetchPriority::from_str(s))
                            .unwrap_or(FetchPriority::Auto);
                        result.preloads.push(PreloadItem {
                            url: resolved.to_string(),
                            kind,
                            priority,
                        });
                    }
                } else if rel_lower.contains("dns-prefetch") {
                    if let Some(href) = attrs.get("href") {
                        let host = extract_host_from_href(href, base_url);
                        if let Some(h) = host
                            && !result.dns_prefetch_hosts.contains(&h)
                        {
                            result.dns_prefetch_hosts.push(h);
                        }
                    }
                } else if rel_lower.contains("preconnect")
                    && let Some(href) = attrs.get("href")
                    && let Ok(resolved) = base_url.resolve(href)
                {
                    let s = resolved.to_string();
                    if !result.preconnect_urls.contains(&s) {
                        result.preconnect_urls.push(s);
                    }
                }
            }
        } else if tag_name.eq_ignore_ascii_case("script") {
            let attrs = parse_tag_attributes(&trimmed_tag[name_end..]);
            if let Some(src) = attrs.get("src")
                && let Ok(resolved) = base_url.resolve(src)
            {
                let priority = attrs
                    .get("fetchpriority")
                    .map(|s| FetchPriority::from_str(s))
                    .unwrap_or(FetchPriority::Auto);
                result.preloads.push(PreloadItem {
                    url: resolved.to_string(),
                    kind: PreloadKind::Script,
                    priority,
                });
            }
        }
    }

    result.sort_by_priority();
    result
}

fn extract_host_from_href(href: &str, base_url: &Url) -> Option<String> {
    let trimmed = href.trim();
    if let Some(rest) = trimmed.strip_prefix("//") {
        let host = rest.split(['/', ':', '?', '#']).next()?;
        Some(host.to_string())
    } else if let Ok(url) = base_url.resolve(trimmed) {
        url.host
    } else {
        None
    }
}

fn parse_tag_attributes(attr_str: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut chars = attr_str.char_indices().peekable();

    while let Some((_, c)) = chars.next() {
        if c.is_whitespace() || c == '/' {
            continue;
        }
        let mut name = String::new();
        name.push(c);
        while let Some(&(_, next_c)) = chars.peek() {
            if next_c.is_whitespace() || next_c == '=' || next_c == '/' || next_c == '>' {
                break;
            }
            chars.next();
            name.push(next_c);
        }

        while let Some(&(_, next_c)) = chars.peek() {
            if next_c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        let mut val = String::new();
        if let Some(&(_, '=')) = chars.peek() {
            chars.next(); // consume '='
            while let Some(&(_, next_c)) = chars.peek() {
                if next_c.is_whitespace() {
                    chars.next();
                } else {
                    break;
                }
            }
            if let Some(&(_, quote)) = chars.peek() {
                if quote == '"' || quote == '\'' {
                    chars.next(); // consume quote
                    for (_, qc) in chars.by_ref() {
                        if qc == quote {
                            break;
                        }
                        val.push(qc);
                    }
                } else {
                    while let Some(&(_, unquoted_c)) = chars.peek() {
                        if unquoted_c.is_whitespace() || unquoted_c == '/' || unquoted_c == '>' {
                            break;
                        }
                        chars.next();
                        val.push(unquoted_c);
                    }
                }
            }
        }

        map.insert(name.to_ascii_lowercase(), val);
    }

    map
}

pub(crate) fn origin_of(url: &crate::url::Url) -> Option<String> {
    let host = url.host.as_deref()?;
    let mut origin = format!("{}://{}", url.scheme, host);
    if let Some(port) = url.port {
        let default = if url.scheme.eq_ignore_ascii_case("https") {
            443
        } else {
            80
        };
        if port != default {
            origin.push_str(&format!(":{port}"));
        }
    }
    Some(origin)
}
