//! # Browser Extension & Add-on API (GAP-023)
//!
//! Provides a Manifest V2/V3 extension system supporting:
//! - Manifest parsing (`manifest.json` with permissions, content scripts, metadata)
//! - Chrome/WHATWG URL match pattern matching (`<all_urls>`, `*://*.example.com/*`, etc.)
//! - Content script injection (CSS stylesheet rules and JavaScript code)
//! - Lifecycle management (`RunAt::DocumentStart`, `DocumentEnd`, `DocumentIdle`)
//! - Add-on registration, enabling/disabling, and isolation

use std::collections::HashMap;

/// When a content script should run relative to document loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunAt {
    DocumentStart,
    #[default]
    DocumentEnd,
    DocumentIdle,
}

/// A parsed Chrome / WebExtension URL match pattern.
///
/// Syntax: `<scheme>://<host>/<path>` or `<all_urls>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchPattern {
    pub raw: String,
    pub match_all: bool,
    pub scheme: Option<String>,
    pub host: Option<String>,
    pub path_prefix: Option<String>,
}

impl MatchPattern {
    /// Parses a URL match pattern string.
    pub fn parse(pattern: &str) -> Option<Self> {
        let p = pattern.trim();
        if p == "<all_urls>" {
            return Some(Self {
                raw: p.to_string(),
                match_all: true,
                scheme: None,
                host: None,
                path_prefix: None,
            });
        }

        let (scheme, rest) = p.split_once("://")?;
        let (host, path) = match rest.split_once('/') {
            Some((h, p)) => (h, p),
            None => (rest, "*"),
        };

        Some(Self {
            raw: pattern.to_string(),
            match_all: false,
            scheme: Some(scheme.to_ascii_lowercase()),
            host: Some(host.to_ascii_lowercase()),
            path_prefix: Some(path.to_string()),
        })
    }

    /// Checks if a given URL matches this pattern.
    pub fn matches_url(&self, url: &str) -> bool {
        if self.match_all {
            return url.starts_with("http://") || url.starts_with("https://");
        }

        let (url_scheme, rest) = match url.split_once("://") {
            Some((s, r)) => (s.to_ascii_lowercase(), r),
            None => return false,
        };

        // Check scheme
        if let Some(ref target_scheme) = self.scheme {
            if target_scheme != "*" && target_scheme != &url_scheme {
                return false;
            }
        }

        let (url_host, url_path) = match rest.split_once('/') {
            Some((h, p)) => (h.to_ascii_lowercase(), format!("/{p}")),
            None => (rest.to_ascii_lowercase(), "/".to_string()),
        };

        // Check host
        if let Some(ref target_host) = self.host {
            if target_host != "*" {
                if let Some(suffix) = target_host.strip_prefix("*.") {
                    if !url_host.ends_with(suffix) && url_host != suffix {
                        return false;
                    }
                } else if &url_host != target_host {
                    return false;
                }
            }
        }

        // Check path prefix
        if let Some(ref target_path) = self.path_prefix {
            if target_path != "*" && !target_path.is_empty() {
                let clean_prefix = target_path.trim_end_matches('*');
                let target_full = if clean_prefix.starts_with('/') {
                    clean_prefix.to_string()
                } else {
                    format!("/{clean_prefix}")
                };
                if !url_path.starts_with(&target_full) {
                    return false;
                }
            }
        }

        true
    }
}

/// An extension content script configuration.
#[derive(Debug, Clone)]
pub struct ContentScript {
    pub matches: Vec<MatchPattern>,
    pub css_code: Vec<String>,
    pub js_code: Vec<String>,
    pub run_at: RunAt,
}

/// Extension manifest definition.
#[derive(Debug, Clone)]
pub struct ExtensionManifest {
    pub name: String,
    pub version: String,
    pub manifest_version: u32,
    pub description: String,
    pub permissions: Vec<String>,
    pub content_scripts: Vec<ContentScript>,
}

impl ExtensionManifest {
    /// Checks whether this extension has permission to access the given URL.
    pub fn has_host_permission(&self, url: &str) -> bool {
        let host_permissions: Vec<&str> = self
            .permissions
            .iter()
            .map(|p| p.trim())
            .filter(|p| p.contains("://") || *p == "<all_urls>")
            .collect();

        // If no explicit host permissions are declared in the manifest, content script
        // URL matches serve as the implicit host permission (Manifest V2/V3 standard).
        if host_permissions.is_empty() {
            return self
                .content_scripts
                .iter()
                .any(|cs| cs.matches.iter().any(|m| m.matches_url(url)));
        }

        host_permissions.iter().any(|p| {
            if let Some(pattern) = MatchPattern::parse(p) {
                pattern.matches_url(url)
            } else {
                false
            }
        })
    }
}

/// Wraps content script code in an isolated world IIFE so it doesn't pollute
/// the page's global variables and provides an isolated `chrome` / `browser` namespace (ARCH-007).
pub fn wrap_isolated_script(js_code: &str, ext_id: &str) -> String {
    format!(
        "(() => {{\n  // Mango Isolated World sandbox: {}\n  const chrome = {{ runtime: {{ id: {:?} }} }};\n  const browser = chrome;\n  try {{\n{}\n  }} catch(e) {{\n    console.error('[Extension {} Error]:', e);\n  }}\n}})();",
        ext_id, ext_id, js_code, ext_id
    )
}

/// A registered browser extension.
#[derive(Debug, Clone)]
pub struct Extension {
    pub id: String,
    pub manifest: ExtensionManifest,
    pub enabled: bool,
}

/// Manages loaded extensions, content scripts, and injection matching.
#[derive(Debug, Clone, Default)]
pub struct ExtensionManager {
    extensions: Vec<Extension>,
}

impl ExtensionManager {
    /// Creates a new, empty extension manager.
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
        }
    }

    /// Registers an extension into the manager.
    pub fn register(&mut self, extension: Extension) {
        self.extensions.retain(|e| e.id != extension.id);
        self.extensions.push(extension);
    }

    /// Loads and registers an extension from a JSON manifest string.
    pub fn load_from_manifest_json(
        &mut self,
        id: impl Into<String>,
        json_str: &str,
    ) -> Result<String, String> {
        let id_str = id.into();
        let manifest = parse_manifest_json(json_str)?;
        let ext = Extension {
            id: id_str.clone(),
            manifest,
            enabled: true,
        };
        self.register(ext);
        Ok(id_str)
    }

    /// Enables an extension by its ID.
    pub fn enable(&mut self, id: &str) -> bool {
        if let Some(ext) = self.extensions.iter_mut().find(|e| e.id == id) {
            ext.enabled = true;
            return true;
        }
        false
    }

    /// Disables an extension by its ID.
    pub fn disable(&mut self, id: &str) -> bool {
        if let Some(ext) = self.extensions.iter_mut().find(|e| e.id == id) {
            ext.enabled = false;
            return true;
        }
        false
    }

    /// Retrieves all registered extensions.
    pub fn extensions(&self) -> &[Extension] {
        &self.extensions
    }

    /// Collects all matching content script CSS code and JS code for a given URL and lifecycle point.
    pub fn get_content_scripts_for_url(
        &self,
        url: &str,
        run_at: RunAt,
    ) -> (Vec<String>, Vec<String>) {
        let mut css_scripts = Vec::new();
        let mut js_scripts = Vec::new();

        for ext in &self.extensions {
            if !ext.enabled {
                continue;
            }

            // Verify host permissions before allowing script execution on this URL (ARCH-007)
            if !ext.manifest.has_host_permission(url) {
                continue;
            }

            for cs in &ext.manifest.content_scripts {
                if cs.run_at != run_at {
                    continue;
                }

                let matches = cs.matches.iter().any(|m| m.matches_url(url));
                if matches {
                    for css in &cs.css_code {
                        css_scripts.push(css.clone());
                    }
                    for js in &cs.js_code {
                        // Inject in an isolated world sandbox (ARCH-007)
                        js_scripts.push(wrap_isolated_script(js, &ext.id));
                    }
                }
            }
        }

        (css_scripts, js_scripts)
    }
}

// ── Lightweight pure-Rust JSON parser for manifest.json ─────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone)]
enum JsonVal {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<JsonVal>),
    Object(HashMap<String, JsonVal>),
}

fn parse_manifest_json(s: &str) -> Result<ExtensionManifest, String> {
    let val = parse_json_value(s.trim())?;
    let obj = match val {
        JsonVal::Object(m) => m,
        _ => return Err("Root manifest must be a JSON object".to_string()),
    };

    let name = obj
        .get("name")
        .and_then(|v| match v { JsonVal::Str(s) => Some(s.clone()), _ => None })
        .unwrap_or_else(|| "Unnamed Extension".to_string());

    let version = obj
        .get("version")
        .and_then(|v| match v { JsonVal::Str(s) => Some(s.clone()), _ => None })
        .unwrap_or_else(|| "1.0.0".to_string());

    let manifest_version = obj
        .get("manifest_version")
        .and_then(|v| match v { JsonVal::Number(n) => Some(*n as u32), _ => None })
        .unwrap_or(3);

    let description = obj
        .get("description")
        .and_then(|v| match v { JsonVal::Str(s) => Some(s.clone()), _ => None })
        .unwrap_or_default();

    let mut permissions = Vec::new();
    if let Some(JsonVal::Array(perms)) = obj.get("permissions") {
        for p in perms {
            if let JsonVal::Str(perm_str) = p {
                permissions.push(perm_str.clone());
            }
        }
    }

    let mut content_scripts = Vec::new();
    if let Some(JsonVal::Array(cs_list)) = obj.get("content_scripts") {
        for cs_val in cs_list {
            if let JsonVal::Object(cs_map) = cs_val {
                let mut patterns = Vec::new();
                if let Some(JsonVal::Array(match_list)) = cs_map.get("matches") {
                    for m in match_list {
                        if let JsonVal::Str(pat_str) = m {
                            if let Some(pat) = MatchPattern::parse(pat_str) {
                                patterns.push(pat);
                            }
                        }
                    }
                }

                let mut css_code = Vec::new();
                if let Some(JsonVal::Array(css_list)) = cs_map.get("css") {
                    for c in css_list {
                        if let JsonVal::Str(s) = c {
                            css_code.push(s.clone());
                        }
                    }
                }

                let mut js_code = Vec::new();
                if let Some(JsonVal::Array(js_list)) = cs_map.get("js") {
                    for j in js_list {
                        if let JsonVal::Str(s) = j {
                            js_code.push(s.clone());
                        }
                    }
                }

                let run_at = match cs_map.get("run_at") {
                    Some(JsonVal::Str(s)) if s == "document_start" => RunAt::DocumentStart,
                    Some(JsonVal::Str(s)) if s == "document_idle" => RunAt::DocumentIdle,
                    _ => RunAt::DocumentEnd,
                };

                content_scripts.push(ContentScript {
                    matches: patterns,
                    css_code,
                    js_code,
                    run_at,
                });
            }
        }
    }

    Ok(ExtensionManifest {
        name,
        version,
        manifest_version,
        description,
        permissions,
        content_scripts,
    })
}

fn parse_json_value(input: &str) -> Result<JsonVal, String> {
    let s = input.trim();
    if s == "null" {
        return Ok(JsonVal::Null);
    }
    if s == "true" {
        return Ok(JsonVal::Bool(true));
    }
    if s == "false" {
        return Ok(JsonVal::Bool(false));
    }
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let unquoted = &s[1..s.len() - 1];
        return Ok(JsonVal::Str(unquoted.replace("\\\"", "\"").replace("\\\\", "\\")));
    }
    if let Ok(num) = s.parse::<f64>() {
        return Ok(JsonVal::Number(num));
    }
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len() - 1].trim();
        if inner.is_empty() {
            return Ok(JsonVal::Array(Vec::new()));
        }
        let items = split_json_items(inner);
        let mut list = Vec::new();
        for item in items {
            list.push(parse_json_value(item.trim())?);
        }
        return Ok(JsonVal::Array(list));
    }
    if s.starts_with('{') && s.ends_with('}') {
        let inner = &s[1..s.len() - 1].trim();
        if inner.is_empty() {
            return Ok(JsonVal::Object(HashMap::new()));
        }
        let items = split_json_items(inner);
        let mut map = HashMap::new();
        for item in items {
            if let Some((k, v)) = split_key_val(item.trim()) {
                let key_str = k.trim().trim_matches('"');
                let val_parsed = parse_json_value(v.trim())?;
                map.insert(key_str.to_string(), val_parsed);
            }
        }
        return Ok(JsonVal::Object(map));
    }

    Err(format!("Invalid JSON token: {s}"))
}

fn split_json_items(s: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth_obj: usize = 0;
    let mut depth_arr: usize = 0;
    let mut in_str = false;
    let mut start = 0;
    let bytes = s.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'"' && (i == 0 || bytes[i - 1] != b'\\') {
            in_str = !in_str;
        } else if !in_str {
            match b {
                b'{' => depth_obj += 1,
                b'}' => depth_obj = depth_obj.saturating_sub(1),
                b'[' => depth_arr += 1,
                b']' => depth_arr = depth_arr.saturating_sub(1),
                b',' if depth_obj == 0 && depth_arr == 0 => {
                    items.push(&s[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
    }
    if start < s.len() {
        items.push(&s[start..]);
    }
    items
}

fn split_key_val(s: &str) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();
    let mut in_str = false;

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'"' && (i == 0 || bytes[i - 1] != b'\\') {
            in_str = !in_str;
        } else if !in_str && b == b':' {
            return Some((&s[..i], &s[i + 1..]));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_pattern_matching() {
        let pat_all = MatchPattern::parse("<all_urls>").unwrap();
        assert!(pat_all.matches_url("https://example.com/test"));
        assert!(pat_all.matches_url("http://localhost:8080/"));
        assert!(!pat_all.matches_url("ftp://example.com"));

        let pat_gh = MatchPattern::parse("https://*.github.com/*").unwrap();
        assert!(pat_gh.matches_url("https://github.com/rust-lang"));
        assert!(pat_gh.matches_url("https://api.github.com/users"));
        assert!(!pat_gh.matches_url("http://github.com/"));
        assert!(!pat_gh.matches_url("https://google.com/"));
    }

    #[test]
    fn test_extension_manager_manifest_and_content_scripts() {
        let manifest_json = r#"{
            "name": "Dark Mode Extension",
            "version": "1.2.0",
            "manifest_version": 3,
            "description": "Applies dark mode styles to websites",
            "permissions": ["storage"],
            "content_scripts": [
                {
                    "matches": ["*://*.example.com/*", "<all_urls>"],
                    "css": ["body { background: #121212 !important; color: #fff !important; }"],
                    "js": ["console.log('Dark mode active');"],
                    "run_at": "document_end"
                }
            ]
        }"#;

        let mut mgr = ExtensionManager::new();
        let ext_id = mgr.load_from_manifest_json("dark-mode", manifest_json).unwrap();
        assert_eq!(ext_id, "dark-mode");
        assert_eq!(mgr.extensions().len(), 1);

        // Matching URL
        let (css, js) = mgr.get_content_scripts_for_url("https://my.example.com/app", RunAt::DocumentEnd);
        assert_eq!(css.len(), 1);
        assert!(css[0].contains("#121212"));
        assert_eq!(js.len(), 1);
        assert!(js[0].contains("Dark mode active"));

        // Disable extension
        mgr.disable("dark-mode");
        let (css_disabled, _) = mgr.get_content_scripts_for_url("https://my.example.com/app", RunAt::DocumentEnd);
        assert!(css_disabled.is_empty(), "Disabled extension should not inject scripts");
    }
}
