//! Persistent browsing history store for Mango Browser.
//!
//! Stores visited URLs, titles, visit timestamps, visit counts, and typed counts.
//! Supports querying by prefix, search term, recency, and frequency, with
//! JSON-backed file persistence across browser sessions.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// A single recorded visit in browsing history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Normalized URL string.
    pub url: String,
    /// Page title recorded during the visit.
    pub title: String,
    /// Timestamp of the last visit in seconds since UNIX epoch.
    pub last_visit_time: u64,
    /// Total number of times this URL was navigated to.
    pub visit_count: u32,
    /// Number of times this URL was explicitly typed into the address bar.
    pub typed_count: u32,
}

impl HistoryEntry {
    /// Creates a new history entry with initial visit count 1.
    pub fn new(url: impl Into<String>, title: impl Into<String>, last_visit_time: u64) -> Self {
        Self {
            url: url.into(),
            title: title.into(),
            last_visit_time,
            visit_count: 1,
            typed_count: 0,
        }
    }
}

/// Persistent history store managing URL visits and query resolution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistoryStore {
    entries: HashMap<String, HistoryEntry>,
}

impl HistoryStore {
    /// Creates an empty history store.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Records a URL navigation event.
    pub fn record_visit(&mut self, url: &str, title: &str) {
        let normalized = normalize_url(url);
        if normalized.is_empty() || normalized == "about:blank" {
            return;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.entries
            .entry(normalized.clone())
            .and_modify(|e| {
                e.last_visit_time = now;
                e.visit_count = e.visit_count.saturating_add(1);
                if !title.is_empty() {
                    e.title = title.to_string();
                }
            })
            .or_insert_with(|| HistoryEntry::new(normalized, title, now));
    }

    /// Records a typed navigation in the address bar (increases typed rank score).
    pub fn record_typed_visit(&mut self, url: &str, title: &str) {
        self.record_visit(url, title);
        let normalized = normalize_url(url);
        if let Some(entry) = self.entries.get_mut(&normalized) {
            entry.typed_count = entry.typed_count.saturating_add(1);
        }
    }

    /// Returns the total number of distinct history entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if history contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Looks up an entry for the given URL.
    pub fn get(&self, url: &str) -> Option<&HistoryEntry> {
        let normalized = normalize_url(url);
        self.entries.get(&normalized)
    }

    /// Removes a specific URL from history.
    pub fn remove(&mut self, url: &str) -> bool {
        let normalized = normalize_url(url);
        self.entries.remove(&normalized).is_some()
    }

    /// Clears all recorded browsing history.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Deletes all history entries visited between `start_secs` and `end_secs` (inclusive).
    pub fn clear_range(&mut self, start_secs: u64, end_secs: u64) {
        self.entries
            .retain(|_, e| e.last_visit_time < start_secs || e.last_visit_time > end_secs);
    }

    /// Returns all history entries ordered from most recent to oldest.
    pub fn recent_entries(&self, limit: usize) -> Vec<HistoryEntry> {
        let mut list: Vec<HistoryEntry> = self.entries.values().cloned().collect();
        list.sort_by_key(|a| std::cmp::Reverse(a.last_visit_time));
        list.truncate(limit);
        list
    }

    /// Returns the most frequently visited entries ordered by visit count descending.
    pub fn top_visited(&self, limit: usize) -> Vec<HistoryEntry> {
        let mut list: Vec<HistoryEntry> = self.entries.values().cloned().collect();
        list.sort_by(|a, b| {
            b.visit_count
                .cmp(&a.visit_count)
                .then_with(|| b.last_visit_time.cmp(&a.last_visit_time))
        });
        list.truncate(limit);
        list
    }

    /// Searches history by keyword query matching against URL or page title.
    pub fn search(&self, query: &str, limit: usize) -> Vec<HistoryEntry> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self.recent_entries(limit);
        }

        let mut list: Vec<HistoryEntry> = self
            .entries
            .values()
            .filter(|e| e.url.to_lowercase().contains(&q) || e.title.to_lowercase().contains(&q))
            .cloned()
            .collect();

        // Rank search results by relevance: exact prefix match > title match > frequency
        list.sort_by(|a, b| {
            let a_url_prefix = a.url.to_lowercase().starts_with(&q);
            let b_url_prefix = b.url.to_lowercase().starts_with(&q);
            b_url_prefix
                .cmp(&a_url_prefix)
                .then_with(|| {
                    let a_title_prefix = a.title.to_lowercase().starts_with(&q);
                    let b_title_prefix = b.title.to_lowercase().starts_with(&q);
                    b_title_prefix.cmp(&a_title_prefix)
                })
                .then_with(|| {
                    (b.visit_count + b.typed_count * 2).cmp(&(a.visit_count + a.typed_count * 2))
                })
                .then_with(|| b.last_visit_time.cmp(&a.last_visit_time))
        });

        list.truncate(limit);
        list
    }

    /// Loads history from a JSON file on disk.
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let file = File::open(path).map_err(|e| format!("Failed to open history file: {e}"))?;
        let reader = BufReader::new(file);
        let store: Self =
            serde_json::from_reader(reader).map_err(|e| format!("Failed to parse history: {e}"))?;
        Ok(store)
    }

    /// Saves history to a JSON file on disk atomically.
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create history directory: {e}"))?;
        }

        let temp_path = PathBuf::from(format!("{}.tmp", path.to_string_lossy()));
        {
            let file = File::create(&temp_path)
                .map_err(|e| format!("Failed to create temp history file: {e}"))?;
            let writer = BufWriter::new(file);
            serde_json::to_writer_pretty(writer, self)
                .map_err(|e| format!("Failed to serialize history: {e}"))?;
        }

        fs::rename(&temp_path, path).map_err(|e| format!("Failed to persist history file: {e}"))?;
        Ok(())
    }
}

/// Normalizes URL by trimming whitespace and trailing slashes for clean indexing.
fn normalize_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.ends_with('/') && trimmed.len() > 1 && !trimmed.ends_with("://") {
        trimmed.trim_end_matches('/').to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_visit_and_frequency() {
        let mut history = HistoryStore::new();
        history.record_visit("https://example.com", "Example Domain");
        history.record_visit("https://example.com/", "Example Domain New Title");

        assert_eq!(history.len(), 1);
        let entry = history.get("https://example.com").expect("entry exists");
        assert_eq!(entry.visit_count, 2);
        assert_eq!(entry.title, "Example Domain New Title");
    }

    #[test]
    fn test_search_and_ranking() {
        let mut history = HistoryStore::new();
        history.record_visit("https://rust-lang.org", "Rust Programming Language");
        history.record_visit("https://crates.io", "Rust Package Registry");
        history.record_visit("https://docs.rs", "Rust Documentation");

        // Bump rust-lang frequency
        history.record_typed_visit("https://rust-lang.org", "Rust Programming Language");

        let results = history.search("rust", 10);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].url, "https://rust-lang.org");
    }

    #[test]
    fn test_clear_range() {
        let mut history = HistoryStore::new();
        history.entries.insert(
            "https://a.com".into(),
            HistoryEntry::new("https://a.com", "A", 100),
        );
        history.entries.insert(
            "https://b.com".into(),
            HistoryEntry::new("https://b.com", "B", 200),
        );
        history.entries.insert(
            "https://c.com".into(),
            HistoryEntry::new("https://c.com", "C", 300),
        );

        history.clear_range(150, 250);
        assert_eq!(history.len(), 2);
        assert!(history.get("https://a.com").is_some());
        assert!(history.get("https://b.com").is_none());
        assert!(history.get("https://c.com").is_some());
    }

    #[test]
    fn test_file_persistence_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!("mango_test_hist_{}", std::process::id()));
        let file_path = temp_dir.join("history.json");

        let mut store = HistoryStore::new();
        store.record_visit("https://mango-browser.org", "Mango Browser");
        store.save_to_file(&file_path).expect("save succeeds");

        let loaded = HistoryStore::load_from_file(&file_path).expect("load succeeds");
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded.get("https://mango-browser.org").unwrap().title,
            "Mango Browser"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
