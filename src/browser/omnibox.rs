//! Omnibox autocomplete and search suggestion engine.
//!
//! Classifies address bar user input into direct URL navigation, search queries,
//! and fuzzy-ranked completions matching user history and bookmarks.

use super::bookmarks::BookmarkStore;
use super::history::HistoryStore;

/// Classification category of an omnibox suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmniboxKind {
    /// Direct URL to navigate to (e.g. domain or scheme).
    DirectUrl,
    /// Web search query via default search provider.
    Search,
    /// Matched from persistent browsing history.
    History,
    /// Matched from saved bookmarks.
    Bookmark,
}

/// A ranked autocomplete suggestion for the address bar dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmniboxSuggestion {
    /// Display title or query label.
    pub title: String,
    /// Complete target destination URL.
    pub url: String,
    /// Kind of match.
    pub kind: OmniboxKind,
    /// Ranking score (higher is ranked first).
    pub score: u32,
}

/// Unified omnibox autocompletion provider.
pub struct OmniboxCompleter;

impl OmniboxCompleter {
    /// Generates ranked suggestions for the current address bar input.
    pub fn complete(
        input: &str,
        history: &HistoryStore,
        bookmarks: &BookmarkStore,
        search_engine_url_template: Option<&str>,
        limit: usize,
    ) -> Vec<OmniboxSuggestion> {
        let text = input.trim();
        if text.is_empty() {
            // Return top frequent/recent history when input is empty
            return history
                .top_visited(limit)
                .into_iter()
                .map(|e| OmniboxSuggestion {
                    title: e.title,
                    url: e.url,
                    kind: OmniboxKind::History,
                    score: 100 + e.visit_count.min(100),
                })
                .collect();
        }

        let mut suggestions = Vec::with_capacity(limit * 2);
        let default_search_template =
            search_engine_url_template.unwrap_or("https://duckduckgo.com/?q={}");

        // 1. Direct URL check
        if is_likely_url(text) {
            let direct_url = to_absolute_url(text);
            suggestions.push(OmniboxSuggestion {
                title: format!("Go to {text}"),
                url: direct_url,
                kind: OmniboxKind::DirectUrl,
                score: 1000,
            });
        }

        // 2. Search query suggestion
        let encoded_query = url_encode(text);
        let search_url = default_search_template.replace("{}", &encoded_query);
        suggestions.push(OmniboxSuggestion {
            title: format!("Search with DuckDuckGo: \"{text}\""),
            url: search_url,
            kind: OmniboxKind::Search,
            score: 500,
        });

        // 3. Bookmark matches (high priority bonus score)
        let matched_bookmarks = bookmarks.search(text);
        for b in matched_bookmarks.into_iter().take(limit) {
            if let Some(url) = b.url() {
                let exact_prefix = url.to_lowercase().starts_with(&text.to_lowercase());
                let bonus = if exact_prefix { 300 } else { 150 };
                suggestions.push(OmniboxSuggestion {
                    title: b.title().to_string(),
                    url: url.to_string(),
                    kind: OmniboxKind::Bookmark,
                    score: 600 + bonus,
                });
            }
        }

        // 4. History matches
        let history_matches = history.search(text, limit);
        for h in history_matches {
            let exact_prefix = h.url.to_lowercase().starts_with(&text.to_lowercase());
            let bonus = if exact_prefix { 200 } else { 50 };
            let freq_bonus = (h.visit_count * 5).min(200);
            let typed_bonus = (h.typed_count * 20).min(300);
            suggestions.push(OmniboxSuggestion {
                title: if h.title.is_empty() {
                    h.url.clone()
                } else {
                    h.title
                },
                url: h.url,
                kind: OmniboxKind::History,
                score: 300 + bonus + freq_bonus + typed_bonus,
            });
        }

        // De-duplicate by destination URL, keeping the highest scored candidate
        let mut seen_urls = std::collections::HashSet::new();
        suggestions.sort_by_key(|a| std::cmp::Reverse(a.score));
        suggestions.retain(|item| seen_urls.insert(item.url.clone()));

        suggestions.truncate(limit);
        suggestions
    }
}

/// Checks whether raw user input looks like a direct URL or domain rather than a search query.
pub fn is_likely_url(input: &str) -> bool {
    let lower = input.trim().to_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mango://")
        || lower.starts_with("file:///")
        || lower.starts_with("about:")
    {
        return true;
    }

    if lower.starts_with("localhost") || lower.starts_with("127.0.0.1") {
        return true;
    }

    // Contains domain TLD pattern (e.g. `example.com`, `docs.rs/foo`) with no internal spaces
    if !lower.contains(' ') && lower.contains('.') {
        let valid_tlds = [
            ".com", ".org", ".net", ".io", ".dev", ".rs", ".edu", ".gov", ".co", ".app", ".ai",
            ".xyz", ".me", ".info", ".html",
        ];
        return valid_tlds.iter().any(|tld| lower.contains(tld));
    }

    false
}

/// Prepends `https://` if no scheme is provided on a direct URL candidate.
pub fn to_absolute_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.contains("://") || trimmed.starts_with("about:") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

/// Percent-encodes search queries.
fn url_encode(query: &str) -> String {
    let mut encoded = String::with_capacity(query.len() * 3);
    for byte in query.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            other => {
                encoded.push_str(&format!("%{:02X}", other));
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_likely_url_detection() {
        assert!(is_likely_url("https://example.com"));
        assert!(is_likely_url("http://localhost:8080"));
        assert!(is_likely_url("rust-lang.org/learn"));
        assert!(is_likely_url("crates.io"));
        assert!(is_likely_url("mango://welcome"));

        // Search queries should not be detected as direct URLs
        assert!(!is_likely_url("rust web browser engine"));
        assert!(!is_likely_url("what is acid3 test"));
    }

    #[test]
    fn test_omnibox_completion_ranking() {
        let mut history = HistoryStore::new();
        history.record_visit("https://crates.io", "Rust Package Registry");

        let mut bookmarks = BookmarkStore::new();
        bookmarks.add_bookmark(Some(1), "Crates", "https://crates.io");

        let results = OmniboxCompleter::complete("crates", &history, &bookmarks, None, 5);

        assert!(!results.is_empty());
        // Crates is both bookmarked and direct URL, highest score wins
        let top = &results[0];
        assert!(top.url.contains("crates.io"));
    }

    #[test]
    fn test_search_query_fallback() {
        let history = HistoryStore::new();
        let bookmarks = BookmarkStore::new();

        let results =
            OmniboxCompleter::complete("acid3 conformance 100/100", &history, &bookmarks, None, 5);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, OmniboxKind::Search);
        assert!(results[0].url.contains("duckduckgo.com"));
    }
}
