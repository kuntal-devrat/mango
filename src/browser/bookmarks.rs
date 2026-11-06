//! Bookmarks store and Netscape Bookmark HTML format importer/exporter.
//!
//! Provides hierarchical bookmark management (folders and items), title/URL searching,
//! JSON profile persistence, and standard Netscape bookmark file interchange format
//! supported by Chrome, Firefox, Safari, and Edge.

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// A node in the bookmark tree hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BookmarkNode {
    /// A single URL bookmark.
    Item {
        id: u64,
        title: String,
        url: String,
        date_added: u64,
        icon: Option<String>,
    },
    /// A bookmark folder containing children.
    Folder {
        id: u64,
        title: String,
        date_added: u64,
        children: Vec<BookmarkNode>,
    },
}

impl BookmarkNode {
    /// Returns the unique ID of this node.
    pub fn id(&self) -> u64 {
        match self {
            BookmarkNode::Item { id, .. } | BookmarkNode::Folder { id, .. } => *id,
        }
    }

    /// Returns the title of this bookmark or folder.
    pub fn title(&self) -> &str {
        match self {
            BookmarkNode::Item { title, .. } | BookmarkNode::Folder { title, .. } => title,
        }
    }

    /// Returns the URL if this node is an Item.
    pub fn url(&self) -> Option<&str> {
        match self {
            BookmarkNode::Item { url, .. } => Some(url),
            BookmarkNode::Folder { .. } => None,
        }
    }

    /// Returns `true` if this node is a folder.
    pub fn is_folder(&self) -> bool {
        matches!(self, BookmarkNode::Folder { .. })
    }
}

/// Hierarchical bookmark store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkStore {
    next_id: u64,
    /// Root folders (e.g. "Bookmarks Bar", "Other Bookmarks").
    pub roots: Vec<BookmarkNode>,
}

impl Default for BookmarkStore {
    fn default() -> Self {
        Self::new()
    }
}

impl BookmarkStore {
    /// Creates a default bookmark store initialized with standard root folders.
    pub fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let bar = BookmarkNode::Folder {
            id: 1,
            title: "Bookmarks Bar".to_string(),
            date_added: now,
            children: Vec::new(),
        };

        let other = BookmarkNode::Folder {
            id: 2,
            title: "Other Bookmarks".to_string(),
            date_added: now,
            children: Vec::new(),
        };

        Self {
            next_id: 3,
            roots: vec![bar, other],
        }
    }

    /// Adds a new bookmark item to a parent folder by ID (or root bar if folder not found).
    pub fn add_bookmark(&mut self, parent_id: Option<u64>, title: &str, url: &str) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let item = BookmarkNode::Item {
            id,
            title: title.to_string(),
            url: url.to_string(),
            date_added: now,
            icon: None,
        };

        let target_parent = parent_id.unwrap_or(1);
        if !self.insert_into_folder(target_parent, item.clone()) {
            // Fallback to first folder
            if let Some(BookmarkNode::Folder { children, .. }) = self.roots.first_mut() {
                children.push(item);
            }
        }

        id
    }

    /// Adds a new subfolder under a parent folder.
    pub fn add_folder(&mut self, parent_id: Option<u64>, title: &str) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let folder = BookmarkNode::Folder {
            id,
            title: title.to_string(),
            date_added: now,
            children: Vec::new(),
        };

        let target_parent = parent_id.unwrap_or(1);
        if !self.insert_into_folder(target_parent, folder.clone()) {
            if let Some(BookmarkNode::Folder { children, .. }) = self.roots.first_mut() {
                children.push(folder);
            }
        }

        id
    }

    fn insert_into_folder(&mut self, folder_id: u64, node: BookmarkNode) -> bool {
        for root in &mut self.roots {
            if Self::insert_recursive(root, folder_id, &node) {
                return true;
            }
        }
        false
    }

    fn insert_recursive(current: &mut BookmarkNode, target_id: u64, node: &BookmarkNode) -> bool {
        if let BookmarkNode::Folder { id, children, .. } = current {
            if *id == target_id {
                children.push(node.clone());
                return true;
            }
            for child in children {
                if Self::insert_recursive(child, target_id, node) {
                    return true;
                }
            }
        }
        false
    }

    /// Removes a bookmark or folder by its ID.
    pub fn remove(&mut self, id: u64) -> bool {
        for root in &mut self.roots {
            if Self::remove_recursive(root, id) {
                return true;
            }
        }
        false
    }

    fn remove_recursive(current: &mut BookmarkNode, target_id: u64) -> bool {
        if let BookmarkNode::Folder { children, .. } = current {
            if let Some(pos) = children.iter().position(|c| c.id() == target_id) {
                children.remove(pos);
                return true;
            }
            for child in children {
                if Self::remove_recursive(child, target_id) {
                    return true;
                }
            }
        }
        false
    }

    /// Finds a bookmark item by its URL.
    pub fn find_by_url(&self, url: &str) -> Option<&BookmarkNode> {
        let target = url.trim();
        for root in &self.roots {
            if let Some(found) = Self::find_url_recursive(root, target) {
                return Some(found);
            }
        }
        None
    }

    fn find_url_recursive<'a>(node: &'a BookmarkNode, url: &str) -> Option<&'a BookmarkNode> {
        match node {
            BookmarkNode::Item { url: u, .. } if u == url => Some(node),
            BookmarkNode::Folder { children, .. } => {
                for c in children {
                    if let Some(res) = Self::find_url_recursive(c, url) {
                        return Some(res);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Searches bookmarks matching keyword in title or URL.
    pub fn search(&self, query: &str) -> Vec<&BookmarkNode> {
        let q = query.trim().to_lowercase();
        let mut results = Vec::new();
        if q.is_empty() {
            return results;
        }

        for root in &self.roots {
            Self::search_recursive(root, &q, &mut results);
        }
        results
    }

    fn search_recursive<'a>(
        node: &'a BookmarkNode,
        query: &str,
        results: &mut Vec<&'a BookmarkNode>,
    ) {
        match node {
            BookmarkNode::Item { title, url, .. } => {
                if title.to_lowercase().contains(query) || url.to_lowercase().contains(query) {
                    results.push(node);
                }
            }
            BookmarkNode::Folder { children, .. } => {
                for c in children {
                    Self::search_recursive(c, query, results);
                }
            }
        }
    }

    /// Exports bookmarks into standard Netscape Bookmark HTML format.
    pub fn export_netscape_html(&self) -> String {
        let mut out = String::from(
            "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n\
            <!-- This is an automatically generated file. It will be read and overwritten. DO NOT EDIT! -->\n\
            <META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n\
            <TITLE>Bookmarks</TITLE>\n\
            <H1>Bookmarks</H1>\n\
            <DL><p>\n",
        );

        for root in &self.roots {
            Self::export_node_html(root, 1, &mut out);
        }

        out.push_str("</DL><p>\n");
        out
    }

    fn export_node_html(node: &BookmarkNode, indent: usize, out: &mut String) {
        let pad = "    ".repeat(indent);
        match node {
            BookmarkNode::Item {
                title,
                url,
                date_added,
                icon,
                ..
            } => {
                let icon_attr = icon
                    .as_ref()
                    .map(|i| format!(" ICON=\"{i}\""))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "{pad}<DT><A HREF=\"{url}\" ADD_DATE=\"{date_added}\"{icon_attr}>{title}</A>\n"
                ));
            }
            BookmarkNode::Folder {
                title,
                date_added,
                children,
                ..
            } => {
                out.push_str(&format!(
                    "{pad}<DT><H3 ADD_DATE=\"{date_added}\">{title}</H3>\n{pad}<DL><p>\n"
                ));
                for child in children {
                    Self::export_node_html(child, indent + 1, out);
                }
                out.push_str(&format!("{pad}</DL><p>\n"));
            }
        }
    }

    /// Imports bookmarks from standard Netscape Bookmark HTML format.
    pub fn import_netscape_html(&mut self, html: &str) -> usize {
        let mut imported_count = 0;
        let mut folder_stack: Vec<u64> = vec![1]; // default to Bookmarks Bar (id 1)

        for line in html.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.contains("</DL>") {
                if folder_stack.len() > 1 {
                    folder_stack.pop();
                }
                continue;
            }

            // Folder start: <DT><H3 ...>Folder Title</H3>
            if let Some(h3_idx) = trimmed.find("<H3") {
                if let Some(title_start) = trimmed[h3_idx..].find('>') {
                    let rest = &trimmed[h3_idx + title_start + 1..];
                    if let Some(title_end) = rest.find("</H3>") {
                        let folder_title = &rest[..title_end];
                        let parent = folder_stack.last().copied();
                        let new_folder_id = self.add_folder(parent, folder_title);
                        folder_stack.push(new_folder_id);
                        continue;
                    }
                }
            }

            // Bookmark item: <DT><A HREF="url" ...>Title</A>
            if let Some(a_idx) = trimmed.find("<A ") {
                let after_a = &trimmed[a_idx + 3..];
                if let Some(href_pos) = after_a.find("HREF=\"") {
                    let after_href = &after_a[href_pos + 6..];
                    if let Some(href_end) = after_href.find('"') {
                        let url = &after_href[..href_end];
                        if let Some(gt) = after_href.find('>') {
                            let after_tag = &after_href[gt + 1..];
                            if let Some(a_close) = after_tag.find("</A>") {
                                let title = &after_tag[..a_close];
                                let parent = folder_stack.last().copied();
                                self.add_bookmark(parent, title, url);
                                imported_count += 1;
                            }
                        }
                    }
                }
            }
        }

        imported_count
    }

    /// Loads bookmarks from disk.
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let file = File::open(path).map_err(|e| format!("Failed to open bookmarks file: {e}"))?;
        let reader = BufReader::new(file);
        let store: Self = serde_json::from_reader(reader)
            .map_err(|e| format!("Failed to parse bookmarks: {e}"))?;
        Ok(store)
    }

    /// Saves bookmarks to disk atomically.
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create bookmarks directory: {e}"))?;
        }

        let temp_path = PathBuf::from(format!("{}.tmp", path.to_string_lossy()));
        {
            let file = File::create(&temp_path)
                .map_err(|e| format!("Failed to create temp bookmarks file: {e}"))?;
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, self)
                .map_err(|e| format!("Failed to serialize bookmarks: {e}"))?;
            writer.flush().map_err(|e| format!("Flush failed: {e}"))?;
        }

        fs::rename(&temp_path, path)
            .map_err(|e| format!("Failed to persist bookmarks file: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_find_bookmark() {
        let mut store = BookmarkStore::new();
        let id = store.add_bookmark(Some(1), "Rust Lang", "https://www.rust-lang.org");
        assert!(id > 0);

        let found = store.find_by_url("https://www.rust-lang.org");
        assert!(found.is_some());
        assert_eq!(found.unwrap().title(), "Rust Lang");

        let search = store.search("rust");
        assert_eq!(search.len(), 1);
        assert_eq!(search[0].id(), id);
    }

    #[test]
    fn test_remove_bookmark() {
        let mut store = BookmarkStore::new();
        let id = store.add_bookmark(Some(1), "Crates.io", "https://crates.io");
        assert!(store.find_by_url("https://crates.io").is_some());

        let removed = store.remove(id);
        assert!(removed);
        assert!(store.find_by_url("https://crates.io").is_none());
    }

    #[test]
    fn test_netscape_html_export_import_roundtrip() {
        let mut store = BookmarkStore::new();
        let folder_id = store.add_folder(Some(1), "Dev Tools");
        store.add_bookmark(Some(folder_id), "GitHub", "https://github.com");
        store.add_bookmark(Some(1), "DuckDuckGo", "https://duckduckgo.com");

        let html = store.export_netscape_html();
        assert!(html.contains("NETSCAPE-Bookmark-file-1"));
        assert!(html.contains("GitHub"));
        assert!(html.contains("https://github.com"));

        let mut imported_store = BookmarkStore::new();
        let count = imported_store.import_netscape_html(&html);
        assert_eq!(count, 2);
        assert!(imported_store.find_by_url("https://github.com").is_some());
        assert!(
            imported_store
                .find_by_url("https://duckduckgo.com")
                .is_some()
        );
    }
}
