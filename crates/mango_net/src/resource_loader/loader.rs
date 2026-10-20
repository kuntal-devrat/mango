use std::sync::Arc;

use super::data_uri::decode_data_uri_bytes;
use super::preload::{PreloadScanResult, origin_of, scan_html_for_preloads};
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
    pub(crate) client: HttpClient,
    pub(crate) cache: Arc<ResourceCache>,
    /// HSTS policy store (RFC 6797) — upgrades `http://` navigations for known hosts.
    pub(crate) hsts: Arc<std::sync::Mutex<crate::security::HstsStore>>,
    /// Active `Content-Security-Policy` policies, keyed by page origin.
    /// Populated as documents load; consulted before every subresource fetch.
    pub(crate) csp_by_origin:
        Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<crate::security::CspPolicy>>>>,
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
    /// and expose the same jar to `document.cookie`.
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
    /// (`<meta charset>` detection, BOM sniffing, and Content-Type charset handling).
    pub(crate) fn decode_text(body: &[u8], content_type: Option<&str>) -> String {
        let encoding = crate::encoding::detect_encoding(body, content_type);
        crate::encoding::decode_with(body, encoding)
    }

    /// Records all `Content-Security-Policy` headers of a just-loaded document.
    pub(crate) fn remember_csp(
        &self,
        page_url: &crate::url::Url,
        headers: &std::collections::HashMap<String, String>,
    ) {
        let security = crate::security::SecurityHeaders::from_headers(headers);
        if security.content_security_policies.is_empty() {
            return;
        }
        if let Some(origin) = origin_of(page_url)
            && let Ok(mut map) = self.csp_by_origin.lock()
        {
            map.insert(origin, security.content_security_policies);
        }
    }

    /// Returns the primary effective CSP for the page a subresource belongs to.
    pub fn csp_for(&self, page_url: &crate::url::Url) -> Option<crate::security::CspPolicy> {
        self.csp_policies_for(page_url).into_iter().next()
    }

    /// Returns all effective CSP policies for the page a subresource belongs to.
    pub fn csp_policies_for(&self, page_url: &crate::url::Url) -> Vec<crate::security::CspPolicy> {
        let Some(origin) = origin_of(page_url) else {
            return Vec::new();
        };
        self.csp_by_origin
            .lock()
            .ok()
            .and_then(|map| map.get(&origin).cloned())
            .unwrap_or_default()
    }

    /// Blocks a subresource when CSP or mixed-content rules forbid it.
    ///
    /// When multiple CSP policies exist, all enforced policies must allow the resource.
    /// `active` is `true` for scripts, styles, and fonts (always blocked on an
    /// HTTPS page when served over HTTP) and `false` for images/media.
    pub(crate) fn guard_subresource(
        &self,
        directive: crate::security::ResourceDirective,
        page_url: &crate::url::Url,
        resource_url: &str,
        active: bool,
    ) -> Result<(), NetworkError> {
        self.guard_subresource_with_nonce(directive, page_url, resource_url, active, None)
    }

    /// Blocks a subresource when CSP or mixed-content rules forbid it, checking both URL and optional nonce.
    pub(crate) fn guard_subresource_with_nonce(
        &self,
        directive: crate::security::ResourceDirective,
        page_url: &crate::url::Url,
        resource_url: &str,
        active: bool,
        nonce: Option<&str>,
    ) -> Result<(), NetworkError> {
        let policies = self.csp_policies_for(page_url);
        if !policies.is_empty() {
            let origin = origin_of(page_url).unwrap_or_default();
            for policy in &policies {
                let allowed_by_url = policy.is_allowed(directive, resource_url, &origin);
                let allowed_by_nonce = nonce.is_some_and(|n| policy.allows_nonce(directive, n));
                if !allowed_by_url && !allowed_by_nonce {
                    log::warn!("CSP blocked {:?} resource {}", directive, resource_url);
                    return Err(NetworkError::Other(format!(
                        "Blocked by Content-Security-Policy: {resource_url}"
                    )));
                }
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

    /// Fetches an HTML document while streaming incoming chunks to `on_chunk`.
    ///
    /// This enables progressive speculative preload scanning and incremental
    /// HTML parsing during network transfer.
    pub fn fetch_document_streaming<F: FnMut(&[u8]) -> bool>(
        &self,
        url: &Url,
        on_chunk: F,
    ) -> Result<FetchedDocument, NetworkError> {
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

        let resp = self.client.fetch_streaming(url, on_chunk)?;

        let content_type = resp.content_type().unwrap_or("text/html").to_string();

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
            self.guard_subresource(
                crate::security::ResourceDirective::Style,
                base_url,
                trimmed,
                true,
            )?;
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

        // 2. Fetch over network as subresource (is_top_level=false with initiator)
        let resp = self
            .client
            .fetch_with_context(&resolved_url, Some(base_url), false, true)?;
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
        self.fetch_script_with_nonce(base_url, src, None)
    }

    /// Fetches an external script with an optional CSP nonce, resolving `src` relative to `base_url`.
    pub fn fetch_script_with_nonce(
        &self,
        base_url: &Url,
        src: &str,
        nonce: Option<&str>,
    ) -> Result<String, NetworkError> {
        let trimmed = src.trim();
        if trimmed.starts_with("data:") {
            self.guard_subresource_with_nonce(
                crate::security::ResourceDirective::Script,
                base_url,
                trimmed,
                true,
                nonce,
            )?;
            let bytes = decode_data_uri_bytes(trimmed)?;
            return Ok(Self::decode_text(&bytes, Some("application/javascript")));
        }

        let resolved_url = base_url
            .resolve(trimmed)
            .map_err(|e| NetworkError::InvalidUrl(e.to_string()))?;
        self.guard_subresource_with_nonce(
            crate::security::ResourceDirective::Script,
            base_url,
            &resolved_url.as_str(),
            true,
            nonce,
        )?;
        let url_key = resolved_url.as_str();

        // 1. Check cache
        if let Some(cached) = self.cache.get(&url_key) {
            return Ok(Self::decode_text(
                &cached.body,
                Some(cached.content_type.as_str()),
            ));
        }

        // 2. Fetch over network as subresource (is_top_level=false with initiator)
        let resp = self
            .client
            .fetch_with_context(&resolved_url, Some(base_url), false, true)?;
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

        // Data URIs are self-contained but must satisfy CSP
        if trimmed.starts_with("data:") {
            self.guard_subresource(
                crate::security::ResourceDirective::Image,
                base_url,
                trimmed,
                false,
            )?;
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

        // 2. Fetch over network as subresource (is_top_level=false with initiator)
        let resp: HttpResponse =
            self.client
                .fetch_with_context(&resolved_url, Some(base_url), false, true)?;
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

        // Data URIs are self-contained but must satisfy CSP
        if trimmed.starts_with("data:") {
            self.guard_subresource(
                crate::security::ResourceDirective::Font,
                base_url,
                trimmed,
                true,
            )?;
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

        // 2. Fetch over network as subresource (is_top_level=false with initiator)
        let resp: HttpResponse =
            self.client
                .fetch_with_context(&resolved_url, Some(base_url), false, true)?;
        if !resp.is_success() {
            return Err(NetworkError::HttpError(resp.status, resp.status_text));
        }

        let content_type = resp.content_type().unwrap_or("font/woff2").to_string();
        let bytes = resp.body.clone();
        if content_type.starts_with("text/html")
            || bytes.starts_with(b"<!DOC")
            || bytes.starts_with(b"<!doc")
            || bytes.starts_with(b"<html")
            || bytes.starts_with(b"<HTML")
        {
            return Err(NetworkError::Other(
                "Received HTML instead of binary font data".into(),
            ));
        }
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

        // Pre-resolve DNS for preconnect and dns-prefetch hosts.
        // Bounded 2s timeout + singleflight: never stall page load on dead DNS.
        if let Some(dns) = dns_cache {
            let dns_timeout = std::time::Duration::from_secs(2);
            for host in &scan.dns_prefetch_hosts {
                let _ = dns.resolve_singleflight(host, 443, dns_timeout);
            }
            for preconnect in &scan.preconnect_urls {
                if let Ok(u) = Url::parse(preconnect)
                    && let Some(h) = &u.host
                {
                    let port = u.port.unwrap_or(if u.scheme == "https" { 443 } else { 80 });
                    let _ = dns.resolve_singleflight(h, port, dns_timeout);
                }
            }
        }

        scan
    }
}
