//! HTTP security policy handling: CSP, HSTS, `X-Content-Type-Options`,
//! mixed-content blocking, and `Referrer-Policy`.
//!
//! The types here are deliberately dependency-free so the same policies can be
//! enforced by the resource loader (for subresources) and by the JS runtime
//! (for `fetch()`/`XMLHttpRequest`).

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Parsed `Content-Security-Policy` (or `Content-Security-Policy-Report-Only`)
/// header, restricted to the directives Mango enforces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CspPolicy {
    /// `default-src`, the fallback for unspecified directives.
    pub default_src: Vec<String>,
    /// `script-src` sources (falls back to `default-src`).
    pub script_src: Vec<String>,
    /// `style-src` sources (falls back to `default-src`).
    pub style_src: Vec<String>,
    /// `img-src` sources (falls back to `default-src`).
    pub img_src: Vec<String>,
    /// `font-src` sources (falls back to `default-src`).
    pub font_src: Vec<String>,
    /// `connect-src` sources for `fetch()`/XHR/WebSocket (falls back to `default-src`).
    pub connect_src: Vec<String>,
    /// `frame-ancestors` — whether this page may be framed.
    pub frame_ancestors: Vec<String>,
    /// `upgrade-insecure-requests` present.
    pub upgrade_insecure_requests: bool,
    /// `block-all-mixed-content` present.
    pub block_all_mixed_content: bool,
    /// Policy was delivered as `Content-Security-Policy-Report-Only`.
    pub report_only: bool,
}

/// The kind of resource a CSP check applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceDirective {
    Script,
    Style,
    Image,
    Font,
    Connect,
}

impl CspPolicy {
    /// Parses a CSP header value. Returns `None` for an empty header.
    pub fn parse(header: &str, report_only: bool) -> Option<CspPolicy> {
        let trimmed = header.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut policy = CspPolicy {
            report_only,
            ..Default::default()
        };

        for directive in trimmed.split(';') {
            let mut parts = directive.split_ascii_whitespace();
            let Some(name) = parts.next() else { continue };
            let sources: Vec<String> = parts.map(|s| s.to_string()).collect();
            match name.to_ascii_lowercase().as_str() {
                "default-src" => policy.default_src = sources,
                "script-src" | "script-src-elem" => {
                    if policy.script_src.is_empty() {
                        policy.script_src = sources;
                    }
                }
                "style-src" | "style-src-elem" => {
                    if policy.style_src.is_empty() {
                        policy.style_src = sources;
                    }
                }
                "img-src" => {
                    if policy.img_src.is_empty() {
                        policy.img_src = sources;
                    }
                }
                "font-src" => {
                    if policy.font_src.is_empty() {
                        policy.font_src = sources;
                    }
                }
                "connect-src" => {
                    if policy.connect_src.is_empty() {
                        policy.connect_src = sources;
                    }
                }
                "frame-ancestors" => policy.frame_ancestors = sources,
                "upgrade-insecure-requests" => policy.upgrade_insecure_requests = true,
                "block-all-mixed-content" => policy.block_all_mixed_content = true,
                _ => {}
            }
        }
        Some(policy)
    }

    /// Returns the source list that governs the given directive kind.
    pub fn sources_for(&self, directive: ResourceDirective) -> &[String] {
        let specific = match directive {
            ResourceDirective::Script => &self.script_src,
            ResourceDirective::Style => &self.style_src,
            ResourceDirective::Image => &self.img_src,
            ResourceDirective::Font => &self.font_src,
            ResourceDirective::Connect => &self.connect_src,
        };
        if specific.is_empty() {
            &self.default_src
        } else {
            specific
        }
    }

    /// Returns true when `url` is allowed for the given directive.
    ///
    /// Supports the `'none'`, `'self'`, `*`, scheme sources (`https:`), host
    /// sources (`https://example.com`), and wildcard subdomains (`*.example.com`).
    pub fn allows(&self, directive: ResourceDirective, url: &str, page_origin: &str) -> bool {
        // A policy with no relevant directive allows everything.
        let sources = self.sources_for(directive);
        if sources.is_empty() && self.default_src.is_empty() {
            return true;
        }

        let url_lower = url.trim().to_ascii_lowercase();
        // Non-network schemes are always permitted (data:, blob:, about:).
        if url_lower.starts_with("data:")
            || url_lower.starts_with("blob:")
            || url_lower.starts_with("about:")
            || url_lower.starts_with('#')
        {
            return true;
        }

        let (url_scheme, url_host) = split_url(&url_lower);
        let (page_scheme, page_host) = split_url(&page_origin.to_ascii_lowercase());

        for source in sources {
            let source = source.trim();
            match source {
                "'none'" => return false,
                "*" => return true,
                "'self'" => {
                    if url_scheme == page_scheme && url_host == page_host {
                        return true;
                    }
                }
                "'unsafe-inline'" | "'unsafe-eval'" | "'wasm-unsafe-eval'" | "'strict-dynamic'" => {
                    // Source-expression keywords do not match URLs.
                }
                "'nonce-'" | "'sha256-'" => {}
                _ => {
                    if let Some(scheme) = source.strip_suffix(':') {
                        if scheme.eq_ignore_ascii_case(&url_scheme) {
                            return true;
                        }
                        continue;
                    }
                    let (src_scheme, src_host) = split_url(&source.to_ascii_lowercase());
                    let host_matches = if let Some(suffix) = src_host.strip_prefix("*.") {
                        url_host == suffix || url_host.ends_with(&format!(".{suffix}"))
                    } else if src_host.is_empty() {
                        // Bare host without scheme: match the host only.
                        let (_, bare_host) =
                            split_url(&format!("//{}", source.to_ascii_lowercase()));
                        url_host == bare_host
                    } else {
                        url_host == src_host
                    };
                    if !host_matches {
                        continue;
                    }
                    let scheme_ok = src_scheme.is_empty()
                        || src_scheme == url_scheme
                        || (src_scheme == "http" && url_scheme == "https");
                    if scheme_ok {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Convenience: is this URL allowed for the given directive on a page?
    pub fn is_allowed(&self, directive: ResourceDirective, url: &str, page_origin: &str) -> bool {
        self.report_only || self.allows(directive, url, page_origin)
    }

    /// True when the policy provides no restrictions at all.
    pub fn is_empty(&self) -> bool {
        self.default_src.is_empty()
            && self.script_src.is_empty()
            && self.style_src.is_empty()
            && self.img_src.is_empty()
            && self.font_src.is_empty()
            && self.connect_src.is_empty()
            && self.frame_ancestors.is_empty()
    }

    /// True when this document must not be embedded in a frame.
    pub fn blocks_framing(&self) -> bool {
        self.frame_ancestors.iter().any(|s| s == "'none'")
    }
}

fn split_url(url: &str) -> (String, String) {
    let after_scheme = match url.find("://") {
        Some(idx) => {
            let scheme = url[..idx].to_string();
            (scheme, &url[idx + 3..])
        }
        None => (String::new(), url),
    };
    let (scheme, rest) = after_scheme;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    (scheme, rest[..end].to_string())
}

/// Security-relevant response headers, parsed into a single struct.
#[derive(Debug, Clone, Default)]
pub struct SecurityHeaders {
    pub content_security_policy: Option<CspPolicy>,
    pub hsts: Option<HstsDirective>,
    pub x_frame_options: Option<String>,
    pub x_content_type_options: Option<String>,
    pub referrer_policy: Option<String>,
}

impl SecurityHeaders {
    /// Extracts security headers from a header map (case-insensitive).
    pub fn from_headers(headers: &HashMap<String, String>) -> SecurityHeaders {
        let get = |name: &str| -> Option<&String> {
            headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v)
        };

        let csp = get("content-security-policy").and_then(|v| CspPolicy::parse(v, false));
        let hsts = get("strict-transport-security").and_then(|v| HstsDirective::parse(v));

        SecurityHeaders {
            content_security_policy: csp,
            hsts,
            x_frame_options: get("x-frame-options").cloned(),
            x_content_type_options: get("x-content-type-options").cloned(),
            referrer_policy: get("referrer-policy").cloned(),
        }
    }

    /// True when responses must not be MIME-sniffed.
    pub fn nosniff(&self) -> bool {
        self.x_content_type_options
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("nosniff"))
    }

    /// True when framing is refused via `X-Frame-Options`.
    pub fn blocks_framing(&self) -> bool {
        self.x_frame_options
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("deny") || v.eq_ignore_ascii_case("sameorigin"))
            || self
                .content_security_policy
                .as_ref()
                .is_some_and(|csp| csp.blocks_framing())
    }

    /// True when the response may only be fetched over HTTPS.
    pub fn requires_https(&self) -> bool {
        self.hsts.as_ref().is_some_and(|h| h.max_age_secs > 0)
    }
}

/// Parsed `Strict-Transport-Security` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HstsDirective {
    pub max_age_secs: u64,
    pub include_subdomains: bool,
    pub preload: bool,
}

impl HstsDirective {
    pub fn parse(value: &str) -> Option<HstsDirective> {
        let mut max_age = 0u64;
        let mut include_subdomains = false;
        let mut preload = false;
        for part in value.split(';') {
            let part = part.trim();
            if let Some(rest) = part
                .strip_prefix("max-age=")
                .or_else(|| part.strip_prefix("Max-Age="))
            {
                max_age = rest.trim().trim_matches('"').parse().unwrap_or(0);
            } else if part.eq_ignore_ascii_case("includeSubDomains") {
                include_subdomains = true;
            } else if part.eq_ignore_ascii_case("preload") {
                preload = true;
            }
        }
        Some(HstsDirective {
            max_age_secs: max_age,
            include_subdomains,
            preload,
        })
    }
}

/// An in-memory HSTS store (RFC 6797) with expiry.
#[derive(Debug, Default)]
pub struct HstsStore {
    entries: HashMap<String, (HstsDirective, Instant)>,
    /// Whether the store should also upgrade subdomains known via `includeSubDomains`.
    subdomains: HashMap<String, bool>,
}

impl HstsStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an HSTS policy for `host`.
    pub fn record(&mut self, host: &str, directive: HstsDirective) {
        let host = host.to_ascii_lowercase();
        if directive.max_age_secs == 0 {
            // max-age=0 removes any existing policy.
            self.entries.remove(&host);
            self.subdomains.remove(&host);
            return;
        }
        let expires = Instant::now() + Duration::from_secs(directive.max_age_secs);
        self.entries.insert(host.clone(), (directive, expires));
        self.subdomains.insert(host, directive.include_subdomains);
    }

    /// Returns the active policy governing `host`, if any.
    pub fn policy_for(&self, host: &str) -> Option<HstsDirective> {
        let host = host.to_ascii_lowercase();
        if let Some((directive, expires)) = self.entries.get(&host) {
            if *expires > Instant::now() {
                return Some(*directive);
            }
            return None;
        }
        // Check ancestors: a policy with includeSubDomains covers subdomains.
        let mut candidate = host.as_str();
        while let Some(dot) = candidate.find('.') {
            candidate = &candidate[dot + 1..];
            if let Some((directive, expires)) = self.entries.get(candidate)
                && *expires > Instant::now()
                && directive.include_subdomains
            {
                return Some(*directive);
            }
        }
        None
    }

    /// True when `http://host/...` must be upgraded to HTTPS before loading.
    pub fn should_upgrade(&self, url: &str) -> bool {
        let lower = url.trim().to_ascii_lowercase();
        if !lower.starts_with("http://") {
            return false;
        }
        let (_, host) = split_url(&lower);
        let host_no_port = host.split(':').next().unwrap_or("").to_string();
        self.policy_for(&host_no_port).is_some()
    }

    /// Number of stored policies (including expired ones awaiting pruning).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Removes expired entries.
    pub fn prune(&mut self) {
        let now = Instant::now();
        self.entries.retain(|_, (_, expires)| *expires > now);
        let active: Vec<String> = self.entries.keys().cloned().collect();
        self.subdomains.retain(|host, _| active.contains(host));
    }
}

/// Returns true when an HTTP subresource on an HTTPS page must be blocked.
///
/// Active mixed content (scripts, stylesheets) is always blocked; passive mixed
/// content (images, media) is upgraded when `upgrade` is set.
pub fn blocks_mixed_content(page_url: &str, resource_url: &str, active: bool) -> bool {
    let page_is_https = page_url.trim().to_ascii_lowercase().starts_with("https://");
    let resource_is_http = resource_url
        .trim()
        .to_ascii_lowercase()
        .starts_with("http://");
    page_is_https && resource_is_http && active
}

/// Upgrades an `http://` URL to `https://` (used for mixed-content upgrade and HSTS).
pub fn upgrade_to_https(url: &str) -> String {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") {
        format!("https://{}", &url[7..])
    } else {
        url.to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Origin, Same-Origin Policy (SOP) & Cross-Origin Resource Sharing (CORS)
// ─────────────────────────────────────────────────────────────────────────────

/// Web origin representation (RFC 6454).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Origin {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

impl Origin {
    /// Parses an origin from a URL string.
    pub fn parse(url: &str) -> Option<Self> {
        let trimmed = url.trim();
        let idx = trimmed.find("://")?;
        let scheme = trimmed[..idx].to_ascii_lowercase();
        let default_port = match scheme.as_str() {
            "https" => 443,
            "http" => 80,
            _ => 0,
        };
        let after_scheme = &trimmed[idx + 3..];
        let host_part = after_scheme
            .split(['/', '?', '#'])
            .next()?
            .to_ascii_lowercase();

        let (host, port) = if let Some(colon) = host_part.rfind(':') {
            let h = host_part[..colon].to_string();
            let p = host_part[colon + 1..]
                .parse::<u16>()
                .unwrap_or(default_port);
            (h, p)
        } else {
            (host_part, default_port)
        };

        if host.is_empty() {
            return None;
        }

        Some(Origin { scheme, host, port })
    }

    /// Serializes the origin as a string (e.g. `https://example.com` or `http://example.com:8080`).
    pub fn serialize(&self) -> String {
        let default_port = match self.scheme.as_str() {
            "https" => 443,
            "http" => 80,
            _ => 0,
        };
        if self.port == default_port || self.port == 0 {
            format!("{}://{}", self.scheme, self.host)
        } else {
            format!("{}://{}:{}", self.scheme, self.host, self.port)
        }
    }

    /// Returns true if two origins are identical per RFC 6454.
    pub fn is_same_origin(&self, other: &Origin) -> bool {
        self.scheme == other.scheme && self.host == other.host && self.port == other.port
    }
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.serialize())
    }
}

/// Checks if two URLs have the same origin per Same-Origin Policy (SOP).
pub fn is_same_origin(url_a: &str, url_b: &str) -> bool {
    match (Origin::parse(url_a), Origin::parse(url_b)) {
        (Some(a), Some(b)) => a.is_same_origin(&b),
        _ => false,
    }
}

/// CORS validation error per W3C Fetch specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorsError {
    /// Origin not allowed by `Access-Control-Allow-Origin`.
    OriginNotAllowed(String),
    /// Request method not allowed by `Access-Control-Allow-Methods`.
    MethodNotAllowed(String),
    /// Request header not allowed by `Access-Control-Allow-Headers`.
    HeaderNotAllowed(String),
    /// Wildcard `*` in `Access-Control-Allow-Origin` is forbidden when credentials are included.
    WildcardOriginWithCredentialsNotAllowed,
    /// Preflight response status was not 2xx.
    PreflightFailed(u16),
    /// Missing `Access-Control-Allow-Origin` response header.
    MissingAllowOriginHeader,
    /// Invalid URL.
    InvalidUrl(String),
}

impl std::fmt::Display for CorsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CorsError::OriginNotAllowed(orig) => write!(
                f,
                "Origin '{orig}' not allowed by Access-Control-Allow-Origin"
            ),
            CorsError::MethodNotAllowed(m) => write!(
                f,
                "Method '{m}' not allowed by Access-Control-Allow-Methods"
            ),
            CorsError::HeaderNotAllowed(h) => write!(
                f,
                "Header '{h}' not allowed by Access-Control-Allow-Headers"
            ),
            CorsError::WildcardOriginWithCredentialsNotAllowed => write!(
                f,
                "Access-Control-Allow-Origin cannot be '*' when request credentials are included"
            ),
            CorsError::PreflightFailed(status) => {
                write!(f, "CORS preflight channel failed with status {status}")
            }
            CorsError::MissingAllowOriginHeader => {
                write!(f, "Missing Access-Control-Allow-Origin response header")
            }
            CorsError::InvalidUrl(msg) => write!(f, "Invalid URL for CORS check: {msg}"),
        }
    }
}

impl std::error::Error for CorsError {}

/// Checks whether an HTTP request qualifies as a CORS "simple request"
/// (requiring no preflight OPTIONS request).
pub fn is_cors_simple_request(method: &str, headers: &HashMap<String, String>) -> bool {
    let method_upper = method.trim().to_ascii_uppercase();
    if !matches!(method_upper.as_str(), "GET" | "HEAD" | "POST") {
        return false;
    }

    for (k, v) in headers {
        let key_lower = k.trim().to_ascii_lowercase();
        match key_lower.as_str() {
            "accept" | "accept-language" | "content-language" => {}
            "content-type" => {
                let media_type = v
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase();
                if !matches!(
                    media_type.as_str(),
                    "application/x-www-form-urlencoded" | "multipart/form-data" | "text/plain"
                ) {
                    return false;
                }
            }
            _ => return false,
        }
    }

    true
}

/// Evaluates CORS preflight response headers.
pub fn validate_cors_preflight(
    preflight_status: u16,
    preflight_headers: &HashMap<String, String>,
    request_origin: &str,
    request_method: &str,
    request_headers: &[&str],
) -> Result<Duration, CorsError> {
    if !(200..300).contains(&preflight_status) {
        return Err(CorsError::PreflightFailed(preflight_status));
    }

    let get_header = |name: &str| -> Option<&str> {
        preflight_headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let allow_origin = get_header("access-control-allow-origin")
        .ok_or(CorsError::MissingAllowOriginHeader)?
        .trim();

    if allow_origin != "*" && !allow_origin.eq_ignore_ascii_case(request_origin) {
        return Err(CorsError::OriginNotAllowed(request_origin.to_string()));
    }

    let req_m = request_method.trim().to_ascii_uppercase();
    if let Some(methods_hdr) = get_header("access-control-allow-methods") {
        let allowed_methods: Vec<String> = methods_hdr
            .split(',')
            .map(|m| m.trim().to_ascii_uppercase())
            .collect();
        if !allowed_methods.iter().any(|m| m == "*" || m == &req_m) {
            return Err(CorsError::MethodNotAllowed(req_m));
        }
    } else if req_m != "GET" && req_m != "HEAD" && req_m != "POST" {
        return Err(CorsError::MethodNotAllowed(req_m));
    }

    if let Some(headers_hdr) = get_header("access-control-allow-headers") {
        let allowed_headers: Vec<String> = headers_hdr
            .split(',')
            .map(|h| h.trim().to_ascii_lowercase())
            .collect();
        for req_h in request_headers {
            let h_lower = req_h.trim().to_ascii_lowercase();
            if !allowed_headers.iter().any(|a| a == "*" || a == &h_lower) {
                return Err(CorsError::HeaderNotAllowed(req_h.to_string()));
            }
        }
    } else if !request_headers.is_empty() {
        return Err(CorsError::HeaderNotAllowed(request_headers[0].to_string()));
    }

    let max_age_secs = get_header("access-control-max-age")
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(5);

    Ok(Duration::from_secs(max_age_secs))
}

/// Evaluates the final response headers of a cross-origin request.
/// Returns the list of response headers exposed to client JavaScript.
pub fn validate_cors_response(
    response_headers: &HashMap<String, String>,
    request_origin: &str,
    credentials: bool,
) -> Result<Vec<String>, CorsError> {
    let get_header = |name: &str| -> Option<&str> {
        response_headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let allow_origin = get_header("access-control-allow-origin")
        .ok_or(CorsError::MissingAllowOriginHeader)?
        .trim();

    if credentials {
        if allow_origin == "*" {
            return Err(CorsError::WildcardOriginWithCredentialsNotAllowed);
        }
        let allow_creds = get_header("access-control-allow-credentials")
            .map(|v| v.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if !allow_creds {
            return Err(CorsError::OriginNotAllowed(request_origin.to_string()));
        }
    }

    if allow_origin != "*" && !allow_origin.eq_ignore_ascii_case(request_origin) {
        return Err(CorsError::OriginNotAllowed(request_origin.to_string()));
    }

    // CORS-safelisted response headers
    let mut exposed = vec![
        "cache-control".to_string(),
        "content-language".to_string(),
        "content-length".to_string(),
        "content-type".to_string(),
        "expires".to_string(),
        "last-modified".to_string(),
        "pragma".to_string(),
    ];

    if let Some(expose_hdr) = get_header("access-control-expose-headers") {
        for name in expose_hdr.split(',') {
            let n = name.trim().to_ascii_lowercase();
            if !n.is_empty() && !exposed.contains(&n) {
                exposed.push(n);
            }
        }
    }

    Ok(exposed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csp_parse_and_self_matching() {
        let policy = CspPolicy::parse(
            "default-src 'self'; script-src 'self' https://cdn.example.com; img-src *",
            false,
        )
        .unwrap();

        assert!(policy.allows(
            ResourceDirective::Script,
            "https://example.com/app.js",
            "https://example.com"
        ));
        assert!(policy.allows(
            ResourceDirective::Script,
            "https://cdn.example.com/lib.js",
            "https://example.com"
        ));
        assert!(!policy.allows(
            ResourceDirective::Script,
            "https://evil.example.net/x.js",
            "https://example.com"
        ));
        // img-src * allows anything
        assert!(policy.allows(
            ResourceDirective::Image,
            "http://any.test/a.png",
            "https://example.com"
        ));
        // Directives fall back to default-src
        assert!(policy.allows(
            ResourceDirective::Font,
            "https://example.com/f.woff2",
            "https://example.com"
        ));
        assert!(!policy.allows(
            ResourceDirective::Connect,
            "https://api.other.com",
            "https://example.com"
        ));
    }

    #[test]
    fn test_csp_none_and_wildcards() {
        let policy = CspPolicy::parse("default-src 'none'", false).unwrap();
        assert!(!policy.allows(
            ResourceDirective::Image,
            "https://example.com/a.png",
            "https://example.com"
        ));

        let wildcard = CspPolicy::parse("script-src *.example.com", false).unwrap();
        assert!(wildcard.allows(
            ResourceDirective::Script,
            "https://sub.example.com/a.js",
            "https://example.com"
        ));
        assert!(wildcard.allows(
            ResourceDirective::Script,
            "https://example.com/a.js",
            "https://example.com"
        ));
        assert!(!wildcard.allows(
            ResourceDirective::Script,
            "https://notexample.com/a.js",
            "https://example.com"
        ));
    }

    #[test]
    fn test_csp_data_urls_allowed() {
        let policy = CspPolicy::parse("img-src 'self'", false).unwrap();
        assert!(policy.allows(
            ResourceDirective::Image,
            "data:image/png;base64,AAAA",
            "https://example.com"
        ));
    }

    #[test]
    fn test_csp_report_only_never_blocks() {
        let policy = CspPolicy::parse("default-src 'none'", true).unwrap();
        assert!(policy.report_only);
        assert!(policy.is_allowed(
            ResourceDirective::Script,
            "https://x.test/a.js",
            "https://example.com"
        ));
    }

    #[test]
    fn test_csp_frame_ancestors() {
        let policy = CspPolicy::parse("frame-ancestors 'none'", false).unwrap();
        assert!(policy.blocks_framing());
        let policy = CspPolicy::parse("frame-ancestors https://example.com", false).unwrap();
        assert!(!policy.blocks_framing());
    }

    #[test]
    fn test_hsts_store_and_upgrade() {
        let mut store = HstsStore::new();
        assert!(!store.should_upgrade("http://example.com/a"));

        store.record(
            "example.com",
            HstsDirective {
                max_age_secs: 3600,
                include_subdomains: true,
                preload: true,
            },
        );
        assert!(store.should_upgrade("http://example.com/a"));
        assert!(store.should_upgrade("http://www.example.com/a"));
        assert!(!store.should_upgrade("http://other.test/a"));
        assert!(!store.should_upgrade("https://example.com/a"));
        assert_eq!(store.policy_for("example.com").unwrap().max_age_secs, 3600);
        assert!(store.policy_for("www.example.com").is_some());
        assert!(store.policy_for("unrelated.test").is_none());
    }

    #[test]
    fn test_hsts_max_age_zero_clears() {
        let mut store = HstsStore::new();
        store.record(
            "example.com",
            HstsDirective {
                max_age_secs: 100,
                include_subdomains: false,
                preload: false,
            },
        );
        assert!(store.should_upgrade("http://example.com/"));
        store.record(
            "example.com",
            HstsDirective {
                max_age_secs: 0,
                include_subdomains: false,
                preload: false,
            },
        );
        assert!(!store.should_upgrade("http://example.com/"));
    }

    #[test]
    fn test_parse_hsts_header() {
        let d = HstsDirective::parse("max-age=63072000; includeSubDomains; preload").unwrap();
        assert_eq!(d.max_age_secs, 63_072_000);
        assert!(d.include_subdomains);
        assert!(d.preload);
    }

    #[test]
    fn test_security_headers_from_map() {
        let mut headers = HashMap::new();
        headers.insert(
            "Content-Security-Policy".to_string(),
            "default-src 'self'".to_string(),
        );
        headers.insert(
            "Strict-Transport-Security".to_string(),
            "max-age=100".to_string(),
        );
        headers.insert("X-Content-Type-Options".to_string(), "nosniff".to_string());
        headers.insert("X-Frame-Options".to_string(), "DENY".to_string());

        let parsed = SecurityHeaders::from_headers(&headers);
        assert!(parsed.nosniff());
        assert!(parsed.blocks_framing());
        assert!(parsed.requires_https());
        assert!(parsed.content_security_policy.is_some());
    }

    #[test]
    fn test_mixed_content_rules() {
        // Active mixed content on an HTTPS page is blocked
        assert!(blocks_mixed_content(
            "https://example.com",
            "http://cdn.test/app.js",
            true
        ));
        // Passive mixed content is allowed (browsers auto-upgrade instead)
        assert!(!blocks_mixed_content(
            "https://example.com",
            "http://cdn.test/a.png",
            false
        ));
        // Plain HTTP pages are unaffected
        assert!(!blocks_mixed_content(
            "http://example.com",
            "http://cdn.test/app.js",
            true
        ));
    }

    #[test]
    fn test_upgrade_to_https() {
        assert_eq!(upgrade_to_https("http://a.test/x"), "https://a.test/x");
        assert_eq!(upgrade_to_https("https://a.test/x"), "https://a.test/x");
        assert_eq!(upgrade_to_https("data:text/plain,hi"), "data:text/plain,hi");
    }

    #[test]
    fn test_same_origin_policy() {
        assert!(is_same_origin(
            "https://example.com/foo",
            "https://example.com/bar"
        ));
        assert!(is_same_origin(
            "https://example.com:443/foo",
            "https://example.com/bar"
        ));
        assert!(is_same_origin(
            "http://example.com:80/foo",
            "http://example.com/bar"
        ));
        // Different schemes
        assert!(!is_same_origin("http://example.com", "https://example.com"));
        // Different hosts
        assert!(!is_same_origin(
            "https://sub.example.com",
            "https://example.com"
        ));
        // Different ports
        assert!(!is_same_origin(
            "https://example.com:8443",
            "https://example.com:443"
        ));

        let orig = Origin::parse("https://mango.dev:9000/path?query").unwrap();
        assert_eq!(orig.scheme, "https");
        assert_eq!(orig.host, "mango.dev");
        assert_eq!(orig.port, 9000);
        assert_eq!(orig.serialize(), "https://mango.dev:9000");
    }

    #[test]
    fn test_cors_simple_request_classification() {
        let mut headers = HashMap::new();
        headers.insert("Accept".to_string(), "*/*".to_string());
        headers.insert("Content-Type".to_string(), "text/plain".to_string());
        assert!(is_cors_simple_request("GET", &headers));
        assert!(is_cors_simple_request("POST", &headers));

        // Non-simple method
        assert!(!is_cors_simple_request("PUT", &headers));
        assert!(!is_cors_simple_request("DELETE", &headers));

        // Non-simple content type
        let mut json_headers = headers.clone();
        json_headers.insert("Content-Type".to_string(), "application/json".to_string());
        assert!(!is_cors_simple_request("POST", &json_headers));

        // Custom header
        let mut custom_headers = headers.clone();
        custom_headers.insert("X-Requested-With".to_string(), "XMLHttpRequest".to_string());
        assert!(!is_cors_simple_request("GET", &custom_headers));
    }

    #[test]
    fn test_cors_preflight_and_response() {
        let mut preflight_headers = HashMap::new();
        preflight_headers.insert(
            "access-control-allow-origin".to_string(),
            "https://app.mango.dev".to_string(),
        );
        preflight_headers.insert(
            "access-control-allow-methods".to_string(),
            "GET, POST, PUT, DELETE".to_string(),
        );
        preflight_headers.insert(
            "access-control-allow-headers".to_string(),
            "content-type, authorization".to_string(),
        );
        preflight_headers.insert("access-control-max-age".to_string(), "600".to_string());

        let max_age = validate_cors_preflight(
            204,
            &preflight_headers,
            "https://app.mango.dev",
            "PUT",
            &["Content-Type", "Authorization"],
        )
        .unwrap();
        assert_eq!(max_age, Duration::from_secs(600));

        // Preflight failed origin
        let err = validate_cors_preflight(
            204,
            &preflight_headers,
            "https://evil.attacker.com",
            "PUT",
            &["Content-Type"],
        )
        .unwrap_err();
        assert!(matches!(err, CorsError::OriginNotAllowed(_)));

        // Preflight failed method
        let err2 = validate_cors_preflight(
            204,
            &preflight_headers,
            "https://app.mango.dev",
            "PATCH",
            &["Content-Type"],
        )
        .unwrap_err();
        assert!(matches!(err2, CorsError::MethodNotAllowed(_)));

        // Preflight failed header
        let err3 = validate_cors_preflight(
            204,
            &preflight_headers,
            "https://app.mango.dev",
            "PUT",
            &["X-Custom-Secret"],
        )
        .unwrap_err();
        assert!(matches!(err3, CorsError::HeaderNotAllowed(_)));

        // Response validation
        let mut resp_headers = HashMap::new();
        resp_headers.insert("access-control-allow-origin".to_string(), "*".to_string());
        resp_headers.insert(
            "access-control-expose-headers".to_string(),
            "X-Total-Count, X-Request-ID".to_string(),
        );
        let exposed =
            validate_cors_response(&resp_headers, "https://app.mango.dev", false).unwrap();
        assert!(exposed.contains(&"x-total-count".to_string()));
        assert!(exposed.contains(&"x-request-id".to_string()));
        assert!(exposed.contains(&"content-type".to_string()));

        // Response validation with credentials rejects wildcard
        let cred_err =
            validate_cors_response(&resp_headers, "https://app.mango.dev", true).unwrap_err();
        assert_eq!(cred_err, CorsError::WildcardOriginWithCredentialsNotAllowed);
    }
}
