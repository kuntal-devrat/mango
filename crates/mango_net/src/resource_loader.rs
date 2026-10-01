//! Resource loader: coordinates document fetching, linked stylesheets, and remote images.

use std::sync::Arc;

use crate::cache::ResourceCache;
use crate::http::{HttpClient, HttpResponse, NetworkError};
use crate::url::Url;

/// Represents a successfully fetched HTML document with its final URL and metadata.
#[derive(Debug, Clone)]
pub struct FetchedDocument {
    pub url: Url,
    pub html: String,
    pub status: u16,
    pub content_type: String,
    pub encoding: crate::encoding::Encoding,
}

/// Coordinates network resource loading for HTML pages, external CSS, and images.
#[derive(Clone)]
pub struct ResourceLoader {
    client: HttpClient,
    cache: Arc<ResourceCache>,
    /// HSTS policy store (RFC 6797) — upgrades `http://` navigations for known hosts.
    hsts: Arc<std::sync::Mutex<crate::security::HstsStore>>,
    /// Active `Content-Security-Policy` policies, keyed by page origin.
    /// Populated as documents load; consulted before every subresource fetch.
    csp_by_origin:
        Arc<std::sync::Mutex<std::collections::HashMap<String, crate::security::CspPolicy>>>,
}

impl Default for ResourceLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceLoader {
    /// Creates a new `ResourceLoader` with standard HTTP client and in-memory cache.
    pub fn new() -> Self {
        Self::with_cookie_jar(Arc::new(std::sync::Mutex::new(
            crate::cookies::CookieJar::new(),
        )))
    }

    /// Creates a `ResourceLoader` backed by a caller-owned cookie jar.
    ///
    /// Passing a shared jar lets the browser persist cookies across restarts
    /// (GAP-016) and expose the same jar to `document.cookie`.
    pub fn with_cookie_jar(jar: Arc<std::sync::Mutex<crate::cookies::CookieJar>>) -> Self {
        Self {
            client: HttpClient::with_cookie_jar(jar),
            cache: Arc::new(ResourceCache::new()),
            hsts: Arc::new(std::sync::Mutex::new(crate::security::HstsStore::new())),
            csp_by_origin: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Returns a handle to the shared cookie jar used for requests.
    pub fn cookie_jar(&self) -> Arc<std::sync::Mutex<crate::cookies::CookieJar>> {
        self.client.cookie_jar.clone()
    }

    /// Decodes a network body using the full charset precedence chain:
    /// BOM → `Content-Type; charset=` → `<meta charset>` prescan → UTF-8.
    ///
    /// Stylesheets, scripts, and documents all follow the same rules as HTML
    /// (PRD Phase 1.4 — `<meta charset>` detection, BOM sniffing, and
    /// Content-Type charset handling).
    fn decode_text(body: &[u8], content_type: Option<&str>) -> String {
        let encoding = crate::encoding::detect_encoding(body, content_type);
        crate::encoding::decode_with(body, encoding)
    }

    /// Records the `Content-Security-Policy` of a just-loaded document.
    fn remember_csp(
        &self,
        page_url: &crate::url::Url,
        headers: &std::collections::HashMap<String, String>,
    ) {
        let security = crate::security::SecurityHeaders::from_headers(headers);
        let Some(policy) = security.content_security_policy else {
            return;
        };
        if let Some(origin) = origin_of(page_url)
            && let Ok(mut map) = self.csp_by_origin.lock()
        {
            map.insert(origin, policy);
        }
    }

    /// Returns the effective CSP for the page a subresource belongs to.
    pub fn csp_for(&self, page_url: &crate::url::Url) -> Option<crate::security::CspPolicy> {
        let origin = origin_of(page_url)?;
        self.csp_by_origin
            .lock()
            .ok()
            .and_then(|map| map.get(&origin).cloned())
    }

    /// Blocks a subresource when CSP or mixed-content rules forbid it.
    ///
    /// `active` is `true` for scripts, styles, and fonts (always blocked on an
    /// HTTPS page when served over HTTP) and `false` for images/media.
    fn guard_subresource(
        &self,
        directive: crate::security::ResourceDirective,
        page_url: &crate::url::Url,
        resource_url: &str,
        active: bool,
    ) -> Result<(), NetworkError> {
        if let Some(policy) = self.csp_for(page_url) {
            let origin = origin_of(page_url).unwrap_or_default();
            if !policy.is_allowed(directive, resource_url, &origin) {
                log::warn!("CSP blocked {:?} resource {}", directive, resource_url);
                return Err(NetworkError::Other(format!(
                    "Blocked by Content-Security-Policy: {resource_url}"
                )));
            }
        }
        if crate::security::blocks_mixed_content(&page_url.as_str(), resource_url, active) {
            log::warn!("Blocked mixed content: {resource_url}");
            return Err(NetworkError::Other(format!(
                "Blocked mixed content: {resource_url}"
            )));
        }
        Ok(())
    }

    /// Records an HSTS policy from a response's `Strict-Transport-Security` header.
    pub fn record_hsts(&self, host: &str, header_value: &str) {
        if let Some(directive) = crate::security::HstsDirective::parse(header_value)
            && let Ok(mut store) = self.hsts.lock()
        {
            store.record(host, directive);
        }
    }

    /// Returns true when `url` should be upgraded to HTTPS because of HSTS.
    pub fn requires_https_upgrade(&self, url: &str) -> bool {
        self.hsts
            .lock()
            .map(|store| store.should_upgrade(url))
            .unwrap_or(false)
    }

    /// Number of active HSTS policies (diagnostics/tests).
    pub fn hsts_policy_count(&self) -> usize {
        self.hsts.lock().map(|s| s.len()).unwrap_or(0)
    }

    /// Fetches an HTML document from the given URL, following redirects.
    pub fn fetch_document(&self, url: &Url) -> Result<FetchedDocument, NetworkError> {
        if url.scheme.eq_ignore_ascii_case("data") {
            let bytes = decode_data_uri_bytes(&url.as_str())?;
            let (html, encoding) =
                crate::encoding::decode_html_bytes_with_encoding(&bytes, Some("text/html"));
            return Ok(FetchedDocument {
                url: url.clone(),
                html,
                status: 200,
                content_type: "text/html".to_string(),
                encoding,
            });
        }

        let url_key = url.as_str();
        if let Some(cached) = self.cache.get(&url_key) {
            let (html, encoding) = crate::encoding::decode_html_bytes_with_encoding(
                &cached.body,
                Some(cached.content_type.as_str()),
            );
            return Ok(FetchedDocument {
                url: url.clone(),
                html,
                status: cached.status,
                content_type: cached.content_type.clone(),
                encoding,
            });
        }

        let resp = self.client.fetch(url)?;

        let content_type = resp.content_type().unwrap_or("text/html").to_string();

        // Record HSTS ONLY over HTTPS (RFC 6797 §8.1)
        if resp.final_url.scheme.eq_ignore_ascii_case("https")
            && let Some(hsts_header) = resp
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("strict-transport-security"))
                .map(|(_, v)| v.clone())
            && let Some(host) = resp.final_url.host.as_deref()
        {
            self.record_hsts(host, &hsts_header);
        }
        let (html, encoding) = crate::encoding::decode_html_bytes_with_encoding(
            &resp.body,
            Some(content_type.as_str()),
        );

        self.cache.insert(
            &url_key,
            &content_type,
            resp.status,
            resp.headers.clone(),
            resp.body.clone(),
            None,
        );

        // Record the document's CSP so subresource fetches can be enforced.
        let final_url = resp.final_url.clone();
        self.remember_csp(&final_url, &resp.headers);

        Ok(FetchedDocument {
            url: final_url,
            html,
            status: resp.status,
            content_type,
            encoding,
        })
    }

    /// Submits a form or data via HTTP POST and returns the fetched document.
    pub fn post_document(
        &self,
        url: &Url,
        body: &[u8],
        content_type: &str,
    ) -> Result<FetchedDocument, NetworkError> {
        let resp = self.client.post_with_body(url, body, content_type)?;

        let content_type = resp.content_type().unwrap_or("text/html").to_string();

        // Record HSTS ONLY over HTTPS (RFC 6797 §8.1)
        if resp.final_url.scheme.eq_ignore_ascii_case("https")
            && let Some(hsts_header) = resp
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("strict-transport-security"))
                .map(|(_, v)| v.clone())
            && let Some(host) = resp.final_url.host.as_deref()
        {
            self.record_hsts(host, &hsts_header);
        }

        let (html, encoding) = crate::encoding::decode_html_bytes_with_encoding(
            &resp.body,
            Some(content_type.as_str()),
        );
        let final_url = resp.final_url.clone();
        self.remember_csp(&final_url, &resp.headers);

        Ok(FetchedDocument {
            url: final_url,
            html,
            status: resp.status,
            content_type,
            encoding,
        })
    }

    /// Fetches an external stylesheet, resolving `href` relative to `base_url`.
    ///
    /// Utilizes the in-memory cache to avoid duplicate downloads of identical stylesheets.
    pub fn fetch_stylesheet(&self, base_url: &Url, href: &str) -> Result<String, NetworkError> {
        let trimmed = href.trim();
        if trimmed.starts_with("data:") {
            let bytes = decode_data_uri_bytes(trimmed)?;
            return Ok(Self::decode_text(&bytes, Some("text/css")));
        }

        let resolved_url = base_url
            .resolve(trimmed)
            .map_err(|e| NetworkError::InvalidUrl(e.to_string()))?;
        self.guard_subresource(
            crate::security::ResourceDirective::Style,
            base_url,
            &resolved_url.as_str(),
            true,
        )?;
        let url_key = resolved_url.as_str();

        // 1. Check cache
        if let Some(cached) = self.cache.get(&url_key) {
            return Ok(Self::decode_text(
                &cached.body,
                Some(cached.content_type.as_str()),
            ));
        }

        // 2. Fetch over network
        let resp = self.client.fetch(&resolved_url)?;
        if !resp.is_success() {
            return Err(NetworkError::HttpError(resp.status, resp.status_text));
        }

        let css_text = Self::decode_text(&resp.body, resp.content_type());
        let content_type = resp.content_type().unwrap_or("text/css").to_string();
        let status = resp.status;
        let headers = resp.headers;
        let body = resp.body;

        // 3. Store in cache
        self.cache
            .insert(&url_key, &content_type, status, headers, body, None);

        Ok(css_text)
    }

    /// Fetches an external script, resolving `src` relative to `base_url`.
    ///
    /// Utilizes the in-memory cache and validates content type to avoid executing HTML 404 pages.
    pub fn fetch_script(&self, base_url: &Url, src: &str) -> Result<String, NetworkError> {
        let trimmed = src.trim();
        if trimmed.starts_with("data:") {
            let bytes = decode_data_uri_bytes(trimmed)?;
            return Ok(Self::decode_text(&bytes, Some("application/javascript")));
        }

        let resolved_url = base_url
            .resolve(trimmed)
            .map_err(|e| NetworkError::InvalidUrl(e.to_string()))?;
        self.guard_subresource(
            crate::security::ResourceDirective::Script,
            base_url,
            &resolved_url.as_str(),
            true,
        )?;
        let url_key = resolved_url.as_str();

        // 1. Check cache
        if let Some(cached) = self.cache.get(&url_key) {
            return Ok(Self::decode_text(
                &cached.body,
                Some(cached.content_type.as_str()),
            ));
        }

        // 2. Fetch over network
        let resp = self.client.fetch(&resolved_url)?;
        if !resp.is_success() {
            return Err(NetworkError::HttpError(resp.status, resp.status_text));
        }

        let ct = resp.content_type().unwrap_or("").to_ascii_lowercase();
        if ct.contains("text/html") {
            return Err(NetworkError::Other(
                "Refusing to execute HTML response as script".to_string(),
            ));
        }

        let script_text = Self::decode_text(&resp.body, resp.content_type());
        let content_type = resp
            .content_type()
            .unwrap_or("application/javascript")
            .to_string();
        let status = resp.status;
        let headers = resp.headers;
        let body = resp.body;

        // 3. Store in cache
        self.cache
            .insert(&url_key, &content_type, status, headers, body, None);

        Ok(script_text)
    }

    /// Fetches image bytes, resolving `src` relative to `base_url`.
    ///
    /// Supports `data:` URIs directly without network calls, and caches remote HTTP/HTTPS images.
    pub fn fetch_image_bytes(&self, base_url: &Url, src: &str) -> Result<Vec<u8>, NetworkError> {
        let trimmed = src.trim();

        // Data URIs are self-contained
        if trimmed.starts_with("data:") {
            return decode_data_uri_bytes(trimmed);
        }

        let resolved_url = base_url
            .resolve(trimmed)
            .map_err(|e| NetworkError::InvalidUrl(e.to_string()))?;
        self.guard_subresource(
            crate::security::ResourceDirective::Image,
            base_url,
            &resolved_url.as_str(),
            false,
        )?;
        let url_key = resolved_url.as_str();

        // 1. Check cache
        if let Some(cached) = self.cache.get(&url_key) {
            return Ok(cached.body);
        }

        // 2. Fetch over network
        let resp: HttpResponse = self.client.fetch(&resolved_url)?;
        if !resp.is_success() {
            return Err(NetworkError::HttpError(resp.status, resp.status_text));
        }

        let content_type = resp.content_type().unwrap_or("image/png").to_string();
        let bytes = resp.body.clone();
        let status = resp.status;
        let headers = resp.headers;
        let body = resp.body;

        // 3. Store in cache
        self.cache
            .insert(&url_key, &content_type, status, headers, body, None);

        Ok(bytes)
    }

    /// Fetches font bytes, resolving `src` relative to `base_url`.
    ///
    /// Supports `data:` URIs directly without network calls, and caches remote HTTP/HTTPS fonts.
    pub fn fetch_font_bytes(&self, base_url: &Url, src: &str) -> Result<Vec<u8>, NetworkError> {
        let trimmed = src.trim();

        // Data URIs are self-contained
        if trimmed.starts_with("data:") {
            return decode_data_uri_bytes(trimmed);
        }

        let resolved_url = base_url
            .resolve(trimmed)
            .map_err(|e| NetworkError::InvalidUrl(e.to_string()))?;
        self.guard_subresource(
            crate::security::ResourceDirective::Font,
            base_url,
            &resolved_url.as_str(),
            true,
        )?;
        let url_key = resolved_url.as_str();

        // 1. Check cache
        if let Some(cached) = self.cache.get(&url_key) {
            return Ok(cached.body);
        }

        // 2. Fetch over network
        let resp: HttpResponse = self.client.fetch(&resolved_url)?;
        if !resp.is_success() {
            return Err(NetworkError::HttpError(resp.status, resp.status_text));
        }

        let content_type = resp.content_type().unwrap_or("font/woff2").to_string();
        let bytes = resp.body.clone();
        let status = resp.status;
        let headers = resp.headers;
        let body = resp.body;

        // 3. Store in cache
        self.cache
            .insert(&url_key, &content_type, status, headers, body, None);

        Ok(bytes)
    }

    /// Access the underlying cache directly.
    pub fn cache(&self) -> &ResourceCache {
        &self.cache
    }

    /// Executes the fast preload scanner on an HTML document (PRD 10.3)
    /// and pre-resolves DNS for preconnect/dns-prefetch targets.
    pub fn scan_and_preload(
        &self,
        html: &str,
        base_url: &Url,
        dns_cache: Option<&crate::dns::DnsCache>,
    ) -> PreloadScanResult {
        let scan = scan_html_for_preloads(html, base_url);

        // Pre-resolve DNS for preconnect and dns-prefetch hosts
        if let Some(dns) = dns_cache {
            for host in &scan.dns_prefetch_hosts {
                let _ = dns.resolve(host, 443);
            }
            for preconnect in &scan.preconnect_urls {
                if let Ok(u) = Url::parse(preconnect)
                    && let Some(h) = &u.host
                {
                    let port = u.port.unwrap_or(if u.scheme == "https" { 443 } else { 80 });
                    let _ = dns.resolve(h, port);
                }
            }
        }

        scan
    }
}

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

/// `scheme://host[:port]` for a URL — the origin used by CSP matching.
fn origin_of(url: &crate::url::Url) -> Option<String> {
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

/// Decode a `data:` URI into raw bytes.
/// Supports both base64-encoded and plain text / percent-encoded data URIs.
/// Format: `data:[<mediatype>][;base64],<data>`
pub fn decode_data_uri_bytes(data_uri: &str) -> Result<Vec<u8>, NetworkError> {
    let rest = data_uri
        .strip_prefix("data:")
        .ok_or_else(|| NetworkError::InvalidUrl("Not a data: URI".to_string()))?;
    if let Some(comma_pos) = rest.find(',') {
        let metadata = &rest[..comma_pos];
        let payload = &rest[comma_pos + 1..];
        let is_base64 = metadata
            .split(';')
            .any(|part| part.trim().eq_ignore_ascii_case("base64"));
        if is_base64 {
            decode_base64_bytes(payload.trim())
                .map_err(|e| NetworkError::Other(format!("Base64 decode error: {e}")))
        } else {
            Ok(decode_percent_bytes(payload))
        }
    } else {
        Err(NetworkError::InvalidUrl(
            "Malformed data: URI — missing comma separator".to_string(),
        ))
    }
}

pub fn decode_percent_bytes(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = (bytes[i + 1] as char).to_digit(16);
            let h2 = (bytes[i + 2] as char).to_digit(16);
            if let (Some(d1), Some(d2)) = (h1, h2) {
                out.push(((d1 << 4) | d2) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

pub fn decode_base64_bytes(input: &str) -> Result<Vec<u8>, &'static str> {
    fn val(c: u8) -> Result<u8, &'static str> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' | b'-' => Ok(62),
            b'/' | b'_' => Ok(63),
            _ => Err("Invalid base64 character"),
        }
    }

    let filtered: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    let mut out = Vec::with_capacity(filtered.len() * 3 / 4);

    let chunks = filtered.chunks(4);
    for chunk in chunks {
        let b0 = val(chunk[0])?;
        let b1 = if chunk.len() > 1 { val(chunk[1])? } else { 0 };
        let b2 = if chunk.len() > 2 { val(chunk[2])? } else { 0 };
        let b3 = if chunk.len() > 3 { val(chunk[3])? } else { 0 };

        out.push((b0 << 2) | (b1 >> 4));
        if chunk.len() > 2 {
            out.push((b1 << 4) | (b2 >> 2));
        }
        if chunk.len() > 3 {
            out.push((b2 << 6) | b3);
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_image_data_uri_returns_bytes_without_network() {
        let loader = ResourceLoader::new();
        let base_url = Url::parse("https://example.com/index.html").unwrap();
        let data_uri = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        let result = loader.fetch_image_bytes(&base_url, data_uri);
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(
            bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "must decode base64 PNG header"
        );
    }

    #[test]
    fn test_fetch_font_data_uri_returns_bytes_without_network() {
        let loader = ResourceLoader::new();
        let base_url = Url::parse("https://example.com/index.html").unwrap();
        let font_data_uri = "data:font/woff2;base64,d09GMgABAAAAA";

        let result = loader.fetch_font_bytes(&base_url, font_data_uri);
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
        assert_ne!(
            bytes,
            font_data_uri.as_bytes(),
            "must decode from ASCII base64"
        );
    }

    #[test]
    fn test_decode_percent_data_uri() {
        let uri = "data:text/plain,Hello%20World%21";
        let bytes = decode_data_uri_bytes(uri).unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), "Hello World!");
    }

    #[test]
    fn test_fetch_with_invalid_url_returns_error() {
        let loader = ResourceLoader::new();
        let base_url = Url::parse("https://example.com/").unwrap();

        let result = loader.fetch_stylesheet(&base_url, "http://[invalid-ipv6");
        assert!(result.is_err());
    }

    #[test]
    fn test_loader_cache_prepopulation() {
        let loader = ResourceLoader::new();
        let url_key = "https://example.com/style.css";
        loader.cache.insert(
            url_key,
            "text/css",
            200,
            std::collections::HashMap::new(),
            b"body { color: red; }".to_vec(),
            None,
        );

        let base_url = Url::parse("https://example.com/page.html").unwrap();
        let css = loader.fetch_stylesheet(&base_url, "style.css").unwrap();
        assert_eq!(css, "body { color: red; }");
    }

    #[test]
    fn test_csp_blocks_disallowed_script_origin() {
        let loader = ResourceLoader::new();
        let page = Url::parse("https://example.com/page").unwrap();
        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "content-security-policy".to_string(),
            "default-src 'self'; script-src 'self' https://cdn.example.com".to_string(),
        );
        loader.remember_csp(&page, &headers);

        assert!(
            loader
                .guard_subresource(
                    crate::security::ResourceDirective::Script,
                    &page,
                    "https://cdn.example.com/app.js",
                    true
                )
                .is_ok(),
            "allow-listed CDN script must load"
        );
        assert!(
            loader
                .guard_subresource(
                    crate::security::ResourceDirective::Script,
                    &page,
                    "https://evil.example.net/x.js",
                    true
                )
                .is_err(),
            "off-origin script must be blocked"
        );
        assert!(
            loader
                .guard_subresource(
                    crate::security::ResourceDirective::Style,
                    &page,
                    "https://example.com/site.css",
                    true
                )
                .is_ok(),
            "'self' allows same-origin stylesheets"
        );
    }

    #[test]
    fn test_mixed_content_is_blocked_on_https_pages() {
        let loader = ResourceLoader::new();
        let page = Url::parse("https://secure.example.com/").unwrap();

        assert!(
            loader
                .guard_subresource(
                    crate::security::ResourceDirective::Script,
                    &page,
                    "http://secure.example.com/legacy.js",
                    true
                )
                .is_err(),
            "active mixed content must be blocked"
        );
        assert!(
            loader
                .guard_subresource(
                    crate::security::ResourceDirective::Image,
                    &page,
                    "http://secure.example.com/pic.png",
                    false
                )
                .is_ok(),
            "passive mixed content is allowed (upgraded elsewhere)"
        );
        // An http:// page has no mixed-content constraint.
        let insecure = Url::parse("http://secure.example.com/").unwrap();
        assert!(
            loader
                .guard_subresource(
                    crate::security::ResourceDirective::Script,
                    &insecure,
                    "http://cdn.example.com/x.js",
                    true
                )
                .is_ok()
        );
    }

    #[test]
    fn test_csp_policies_are_keyed_per_origin() {
        let loader = ResourceLoader::new();
        let a = Url::parse("https://a.example/").unwrap();
        let b = Url::parse("https://b.example/").unwrap();
        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "content-security-policy".to_string(),
            "script-src 'none'".to_string(),
        );
        loader.remember_csp(&a, &headers);

        assert!(
            loader
                .guard_subresource(
                    crate::security::ResourceDirective::Script,
                    &a,
                    "https://a.example/x.js",
                    true
                )
                .is_err(),
            "a.example is locked down"
        );
        assert!(
            loader
                .guard_subresource(
                    crate::security::ResourceDirective::Script,
                    &b,
                    "https://b.example/x.js",
                    true
                )
                .is_ok(),
            "b.example has no policy yet"
        );
    }

    #[test]
    fn test_charset_detection_decodes_stylesheets() {
        // A <meta charset> prescan (the HTML5 first-1024-byte rule) decides it.
        let mut body: Vec<u8> = b"<meta charset=\"iso-8859-1\">/* caf".to_vec();
        body.push(0xE9); // é in latin-1
        body.extend_from_slice(b" */\nbody{color:red}");
        let text = ResourceLoader::decode_text(&body, None);
        assert!(text.contains("body{color:red}"));
        assert!(
            text.contains('é'),
            "meta-declared latin-1 byte 0xE9 must decode to U+00E9, got: {text}"
        );

        // Content-Type charset wins over the default.
        let latin = b"/* caf\xe9 */ body{}".to_vec();
        let text = ResourceLoader::decode_text(&latin, Some("text/css; charset=iso-8859-1"));
        assert!(text.contains('é'), "got: {text}");

        // UTF-8 BOM is honored and stripped.
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice("body{content:'✓'}".as_bytes());
        let text = ResourceLoader::decode_text(&bom, None);
        assert!(text.contains('✓'), "got: {text:?}");
        assert!(!text.starts_with('\u{feff}'), "BOM stripped: {text:?}");

        // Without a declaration, invalid UTF-8 does not panic — it degrades
        // to U+FFFD exactly like a browser's fallback decoder.
        let text = ResourceLoader::decode_text(b"a\xffb", None);
        assert!(
            text.starts_with('a') && text.ends_with('b'),
            "got: {text:?}"
        );
    }

    #[test]
    fn test_preload_scanner_and_priority_hints() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <link rel="dns-prefetch" href="//cdn.example.com">
                <link rel="preconnect" href="https://api.example.com">
                <link rel="preload" href="/fonts/inter.woff2" as="font" fetchpriority="high">
                <link rel="preload" href="/css/theme.css" as="style">
                <link rel="preload" href="/img/hero.webp" as="image" fetchpriority="low">
                <script src="/js/app.js" fetchpriority="high"></script>
            </head>
            <body></body>
            </html>
        "#;

        let base_url = Url::parse("https://example.com/index.html").unwrap();
        let result = scan_html_for_preloads(html, &base_url);

        assert_eq!(result.dns_prefetch_hosts, vec!["cdn.example.com"]);
        assert_eq!(result.preconnect_urls, vec!["https://api.example.com/"]);

        assert_eq!(result.preloads.len(), 4);
        // High priority first
        assert_eq!(result.preloads[0].priority, FetchPriority::High);
        assert_eq!(result.preloads[1].priority, FetchPriority::High);
        // Auto second
        assert_eq!(result.preloads[2].priority, FetchPriority::Auto);
        // Low third
        assert_eq!(result.preloads[3].priority, FetchPriority::Low);
        assert_eq!(result.preloads[3].url, "https://example.com/img/hero.webp");
    }
}
