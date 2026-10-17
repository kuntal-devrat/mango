//! HTTP security policy handling: CSP, HSTS, `X-Content-Type-Options`,
//! mixed-content blocking, and `Referrer-Policy`.
//!
//! The types here are deliberately dependency-free so the same policies can be
//! enforced by the resource loader (for subresources) and by the JS runtime
//! (for `fetch()`/`XMLHttpRequest`).

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Parsed `Content-Security-Policy` (or `Content-Security-Policy-Report-Only`)
/// header conforming to W3C CSP Level 3.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CspPolicy {
    /// `default-src`, the fallback for unspecified fetch directives.
    pub default_src: Vec<String>,
    /// `script-src` sources (falls back to `default-src`).
    pub script_src: Vec<String>,
    /// `script-src-elem` sources (falls back to `script-src` -> `default-src`).
    pub script_src_elem: Vec<String>,
    /// `script-src-attr` sources (falls back to `script-src` -> `default-src`).
    pub script_src_attr: Vec<String>,
    /// `style-src` sources (falls back to `default-src`).
    pub style_src: Vec<String>,
    /// `style-src-elem` sources (falls back to `style-src` -> `default-src`).
    pub style_src_elem: Vec<String>,
    /// `style-src-attr` sources (falls back to `style-src` -> `default-src`).
    pub style_src_attr: Vec<String>,
    /// `img-src` sources (falls back to `default-src`).
    pub img_src: Vec<String>,
    /// `font-src` sources (falls back to `default-src`).
    pub font_src: Vec<String>,
    /// `connect-src` sources for `fetch()`/XHR/WebSocket (falls back to `default-src`).
    pub connect_src: Vec<String>,
    /// `object-src` sources for <object>, <embed>, <applet> (falls back to `default-src`).
    pub object_src: Vec<String>,
    /// `media-src` sources for <video>, <audio> (falls back to `default-src`).
    pub media_src: Vec<String>,
    /// `frame-src` sources for <iframe> and <frame> (falls back to `child-src` -> `default-src`).
    pub frame_src: Vec<String>,
    /// `child-src` sources for workers and frames (falls back to `default-src`).
    pub child_src: Vec<String>,
    /// `worker-src` sources for DedicatedWorker, SharedWorker, ServiceWorker (falls back to `child-src` -> `script-src` -> `default-src`).
    pub worker_src: Vec<String>,
    /// `manifest-src` sources for application manifests (falls back to `default-src`).
    pub manifest_src: Vec<String>,
    /// `base-uri` sources for <base href="..."> (does not fall back to `default-src`).
    pub base_uri: Vec<String>,
    /// `form-action` sources for <form action="..."> (does not fall back to `default-src`).
    pub form_action: Vec<String>,
    /// `frame-ancestors` — whether this page may be framed.
    pub frame_ancestors: Vec<String>,
    /// `report-uri` endpoints for violation reports.
    pub report_uri: Vec<String>,
    /// `report-to` Reporting API group name.
    pub report_to: Option<String>,
    /// `upgrade-insecure-requests` present.
    pub upgrade_insecure_requests: bool,
    /// `block-all-mixed-content` present.
    pub block_all_mixed_content: bool,
    /// Policy was delivered as `Content-Security-Policy-Report-Only`.
    pub report_only: bool,
}

/// The kind of resource or execution context a CSP check applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceDirective {
    Script,
    ScriptElem,
    ScriptAttr,
    Style,
    StyleElem,
    StyleAttr,
    Image,
    Font,
    Connect,
    Object,
    Media,
    Frame,
    Child,
    Worker,
    Manifest,
    BaseUri,
    FormAction,
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
                "script-src" => policy.script_src = sources,
                "script-src-elem" => policy.script_src_elem = sources,
                "script-src-attr" => policy.script_src_attr = sources,
                "style-src" => policy.style_src = sources,
                "style-src-elem" => policy.style_src_elem = sources,
                "style-src-attr" => policy.style_src_attr = sources,
                "img-src" => policy.img_src = sources,
                "font-src" => policy.font_src = sources,
                "connect-src" => policy.connect_src = sources,
                "object-src" => policy.object_src = sources,
                "media-src" => policy.media_src = sources,
                "frame-src" => policy.frame_src = sources,
                "child-src" => policy.child_src = sources,
                "worker-src" => policy.worker_src = sources,
                "manifest-src" => policy.manifest_src = sources,
                "base-uri" => policy.base_uri = sources,
                "form-action" => policy.form_action = sources,
                "frame-ancestors" => policy.frame_ancestors = sources,
                "report-uri" => policy.report_uri = sources,
                "report-to" => policy.report_to = sources.into_iter().next(),
                "upgrade-insecure-requests" => policy.upgrade_insecure_requests = true,
                "block-all-mixed-content" => policy.block_all_mixed_content = true,
                _ => {}
            }
        }
        Some(policy)
    }

    /// Returns the source list that governs the given directive kind, observing CSP Level 3 fallback rules.
    pub fn sources_for(&self, directive: ResourceDirective) -> &[String] {
        match directive {
            ResourceDirective::Script => {
                if !self.script_src.is_empty() {
                    &self.script_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::ScriptElem => {
                if !self.script_src_elem.is_empty() {
                    &self.script_src_elem
                } else if !self.script_src.is_empty() {
                    &self.script_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::ScriptAttr => {
                if !self.script_src_attr.is_empty() {
                    &self.script_src_attr
                } else if !self.script_src.is_empty() {
                    &self.script_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::Style => {
                if !self.style_src.is_empty() {
                    &self.style_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::StyleElem => {
                if !self.style_src_elem.is_empty() {
                    &self.style_src_elem
                } else if !self.style_src.is_empty() {
                    &self.style_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::StyleAttr => {
                if !self.style_src_attr.is_empty() {
                    &self.style_src_attr
                } else if !self.style_src.is_empty() {
                    &self.style_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::Image => {
                if !self.img_src.is_empty() {
                    &self.img_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::Font => {
                if !self.font_src.is_empty() {
                    &self.font_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::Connect => {
                if !self.connect_src.is_empty() {
                    &self.connect_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::Object => {
                if !self.object_src.is_empty() {
                    &self.object_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::Media => {
                if !self.media_src.is_empty() {
                    &self.media_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::Frame => {
                if !self.frame_src.is_empty() {
                    &self.frame_src
                } else if !self.child_src.is_empty() {
                    &self.child_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::Child => {
                if !self.child_src.is_empty() {
                    &self.child_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::Worker => {
                if !self.worker_src.is_empty() {
                    &self.worker_src
                } else if !self.child_src.is_empty() {
                    &self.child_src
                } else if !self.script_src.is_empty() {
                    &self.script_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::Manifest => {
                if !self.manifest_src.is_empty() {
                    &self.manifest_src
                } else {
                    &self.default_src
                }
            }
            ResourceDirective::BaseUri => &self.base_uri,
            ResourceDirective::FormAction => &self.form_action,
        }
    }

    /// Returns true when `url` is allowed for the given directive.
    ///
    /// Supports the `'none'`, `'self'`, `*`, scheme sources (`https:`), host
    /// sources (`https://example.com`), and wildcard subdomains (`*.example.com`).
    /// `data:`/`blob:`/`about:` URLs require an explicit matching scheme
    /// source (e.g. `img-src data:`) and are never allowed by `*` or `'self'`.
    /// Host sources also compare port (when specified) and path prefix.
    pub fn allows(&self, directive: ResourceDirective, url: &str, page_origin: &str) -> bool {
        // A policy with no relevant directive allows everything.
        let sources = self.sources_for(directive);
        if sources.is_empty() && self.default_src.is_empty() {
            return true;
        }

        // CSP spec: 'none' matches nothing and denies all requests.
        // It must be checked before evaluating permissive sources like `*`.
        if sources
            .iter()
            .any(|s| s.trim().eq_ignore_ascii_case("'none'"))
        {
            return false;
        }

        let url_trimmed = url.trim();

        let url_lower = url_trimmed.to_ascii_lowercase();
        // Same-document fragment references are always allowed.
        if url_lower.starts_with('#') {
            return true;
        }

        // Extract scheme for non-hierarchical URLs (data:, blob:, about:, ...).
        let url_scheme = extract_scheme(&url_lower).unwrap_or_default();
        if matches!(
            url_scheme.as_str(),
            "data" | "blob" | "about" | "filesystem" | "javascript"
        ) {
            // Require an explicit scheme source; `*` and `'self'` do not match.
            for source in sources {
                let s = source.trim();
                if s.eq_ignore_ascii_case(&format!("{url_scheme}:")) {
                    return true;
                }
                // `'none'` denies everything.
                if s == "'none'" {
                    return false;
                }
            }
            return false;
        }

        let url_parts = parse_csp_url(&url_lower);
        let page_parts = parse_csp_url(&page_origin.to_ascii_lowercase());

        for source in sources {
            let source = source.trim();
            match source {
                "'none'" => return false,
                "*" => {
                    // `*` matches any network URL (not data:/blob:, handled above).
                    return true;
                }
                "'self'" => {
                    if csp_self_matches(&url_parts, &page_parts) {
                        return true;
                    }
                }
                "'unsafe-inline'" | "'unsafe-eval'" | "'wasm-unsafe-eval'" | "'strict-dynamic'" => {
                    // Source-expression keywords do not match URLs.
                }
                "'nonce-'" | "'sha256-'" => {}
                _ => {
                    if let Some(scheme) = source.strip_suffix(':') {
                        // Scheme source like `https:` / `data:`.
                        if !scheme.contains('/') && !scheme.contains('.') && is_csp_scheme(scheme) {
                            if scheme.eq_ignore_ascii_case(&url_parts.scheme) {
                                return true;
                            }
                            continue;
                        }
                    }
                    let src = parse_csp_url(&source.to_ascii_lowercase());
                    // Wildcard subdomain: `*.example.com` (optionally with scheme/port/path).
                    let host_to_check = src.host.strip_prefix("*.").map(|s| s.to_string());
                    let host_matches = if let Some(suffix) = host_to_check {
                        // `*.example.com` matches `example.com` and `*.example.com`.
                        url_parts.host == suffix
                            || (!url_parts.host.is_empty()
                                && url_parts.host.ends_with(&format!(".{suffix}")))
                    } else if src.host.is_empty() {
                        // Bare host without scheme (e.g. `example.com` or `example.com:8080/path`):
                        // parse by prepending a scheme correctly.
                        let with_scheme = format!("https://{}", source.to_ascii_lowercase());
                        let bare = parse_csp_url(&with_scheme);
                        if bare.host.is_empty() {
                            continue;
                        }
                        if url_parts.host != bare.host {
                            continue;
                        }
                        // Fall through to port/path checks using `bare`.
                        if let Some(p) = bare.port {
                            if url_parts.port != Some(p) {
                                continue;
                            }
                        }
                        if !bare.path.is_empty() && bare.path != "/" {
                            if !url_parts.path.starts_with(&bare.path) {
                                continue;
                            }
                        }
                        // Scheme unrestricted for bare hosts.
                        return true;
                    } else {
                        url_parts.host == src.host
                    };
                    if !host_matches {
                        continue;
                    }
                    let scheme_ok = src.scheme.is_empty()
                        || src.scheme == url_parts.scheme
                        || (src.scheme == "http" && url_parts.scheme == "https");
                    if !scheme_ok {
                        continue;
                    }
                    // Port must match when the source pins one.
                    if let Some(p) = src.port {
                        let url_port = url_parts
                            .port
                            .or_else(|| default_port_for(&url_parts.scheme));
                        if url_port != Some(p) {
                            continue;
                        }
                    }
                    // Path prefix must match when the source includes a path.
                    if !src.path.is_empty() && src.path != "/" {
                        if !url_parts.path.starts_with(&src.path) {
                            continue;
                        }
                    }
                    return true;
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
            && self.script_src_elem.is_empty()
            && self.script_src_attr.is_empty()
            && self.style_src.is_empty()
            && self.style_src_elem.is_empty()
            && self.style_src_attr.is_empty()
            && self.img_src.is_empty()
            && self.font_src.is_empty()
            && self.connect_src.is_empty()
            && self.object_src.is_empty()
            && self.media_src.is_empty()
            && self.frame_src.is_empty()
            && self.child_src.is_empty()
            && self.worker_src.is_empty()
            && self.manifest_src.is_empty()
            && self.base_uri.is_empty()
            && self.form_action.is_empty()
            && self.frame_ancestors.is_empty()
    }

    /// True when this document must not be embedded in a frame.
    pub fn blocks_framing(&self) -> bool {
        self.frame_ancestors.iter().any(|s| s == "'none'")
    }

    /// Checks if an inline script/style or resource is permitted by a nonce source (e.g. `'nonce-xyz'`).
    pub fn allows_nonce(&self, directive: ResourceDirective, nonce: &str) -> bool {
        let sources = self.sources_for(directive);
        if sources.is_empty() && self.default_src.is_empty() {
            return true;
        }
        let nonce_clean = nonce.trim();
        if nonce_clean.is_empty() {
            return false;
        }
        for source in sources {
            let s = source.trim();
            if let Some(rest) = s.strip_prefix("'nonce-").and_then(|r| r.strip_suffix('\'')) {
                if rest == nonce_clean {
                    return true;
                }
            }
        }
        false
    }

    /// Checks if content digest matches an allowed cryptographic hash source (e.g. `'sha256-...'`).
    pub fn allows_hash(&self, directive: ResourceDirective, algo: SriAlgorithm, hash_bytes: &[u8]) -> bool {
        let sources = self.sources_for(directive);
        if sources.is_empty() && self.default_src.is_empty() {
            return true;
        }
        let b64 = crate::websocket::base64_encode(hash_bytes);
        let prefix = match algo {
            SriAlgorithm::Sha256 => "'sha256-",
            SriAlgorithm::Sha384 => "'sha384-",
            SriAlgorithm::Sha512 => "'sha512-",
        };
        for source in sources {
            let s = source.trim();
            if s.to_ascii_lowercase().starts_with(prefix) && s.ends_with('\'') {
                let expected = &s[prefix.len()..s.len() - 1];
                if expected == b64 {
                    return true;
                }
            }
        }
        false
    }

    /// Computes cryptographic hashes of `content` and returns true if any match an allowed hash directive.
    pub fn allows_content_hash(&self, directive: ResourceDirective, content: &[u8]) -> bool {
        self.allows_hash(directive, SriAlgorithm::Sha256, &sha256(content))
            || self.allows_hash(directive, SriAlgorithm::Sha384, &sha384(content))
            || self.allows_hash(directive, SriAlgorithm::Sha512, &sha512(content))
    }
}

/// Content Security Policy violation report conforming to W3C CSP Level 3 / Reporting API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CspViolationReport {
    pub document_uri: String,
    pub referrer: String,
    pub blocked_uri: String,
    pub effective_directive: String,
    pub original_policy: String,
    pub disposition: String,
    pub status_code: u16,
    pub sample: Option<String>,
}

impl CspViolationReport {
    /// Serializes report into standard JSON payload format `{"csp-report": { ... }}`.
    pub fn to_json(&self) -> String {
        let sample_json = match &self.sample {
            Some(s) => format!(",\"script-sample\":\"{}\"", escape_json_string(s)),
            None => String::new(),
        };
        format!(
            "{{\"csp-report\":{{\"document-uri\":\"{}\",\"referrer\":\"{}\",\"blocked-uri\":\"{}\",\"effective-directive\":\"{}\",\"original-policy\":\"{}\",\"disposition\":\"{}\",\"status-code\":{}{}}}}}",
            escape_json_string(&self.document_uri),
            escape_json_string(&self.referrer),
            escape_json_string(&self.blocked_uri),
            escape_json_string(&self.effective_directive),
            escape_json_string(&self.original_policy),
            escape_json_string(&self.disposition),
            self.status_code,
            sample_json,
        )
    }
}

fn escape_json_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
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
    // Strip port for legacy callers (use parse_csp_url for port-aware matching).
    let host_port = &rest[..end];
    let host = host_port
        .rsplit_once(':')
        .and_then(|(h, p)| {
            if p.chars().all(|c| c.is_ascii_digit()) && !h.is_empty() {
                Some(h)
            } else {
                None
            }
        })
        .unwrap_or(host_port);
    (scheme, host.to_string())
}

/// Parsed CSP URL parts with port and path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CspUrlParts {
    scheme: String,
    host: String,
    port: Option<u16>,
    path: String,
}

fn extract_scheme(url_lower: &str) -> Option<String> {
    let before_colon = url_lower.split([':', '/', '?', '#', ' ']).next()?;
    let colon_pos = url_lower.find(':')?;
    let candidate = &url_lower[..colon_pos];
    if candidate.is_empty()
        || candidate.contains('/')
        || candidate.contains('?')
        || candidate.contains('#')
        || candidate.contains(' ')
    {
        return None;
    }
    // Must be valid scheme characters.
    if !candidate
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
    {
        return None;
    }
    if !candidate
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    {
        return None;
    }
    let _ = before_colon;
    Some(candidate.to_string())
}

fn is_csp_scheme(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
}

fn default_port_for(scheme: &str) -> Option<u16> {
    match scheme {
        "http" | "ws" => Some(80),
        "https" | "wss" => Some(443),
        "ftp" => Some(21),
        _ => None,
    }
}

/// Parses a URL/source into scheme/host/port/path for CSP matching.
fn parse_csp_url(url: &str) -> CspUrlParts {
    let mut parts = CspUrlParts::default();
    let s = url.trim();
    // Split scheme.
    let (scheme, rest) = match s.find("://") {
        Some(idx) => (s[..idx].to_string(), s[idx + 3..].to_string()),
        None => {
            // Check for non-hierarchical `scheme:rest`.
            if let Some(colon) = s.find(':') {
                let cand = &s[..colon];
                if !cand.is_empty()
                    && !cand.contains('/')
                    && !cand.contains('?')
                    && !cand.contains('#')
                    && !cand.contains(' ')
                    && is_csp_scheme(cand)
                {
                    // e.g. `data:image/...` — no host.
                    parts.scheme = cand.to_string();
                    parts.path = s[colon + 1..].to_string();
                    return parts;
                }
            }
            (String::new(), s.to_string())
        }
    };
    parts.scheme = scheme;
    // Split authority vs path/query/fragment.
    let auth_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..auth_end];
    parts.path = if auth_end < rest.len() {
        let p = &rest[auth_end..];
        // Strip query/fragment, keep path.
        let path_end = p.find(['?', '#']).unwrap_or(p.len());
        let mut path = p[..path_end].to_string();
        if path.is_empty() {
            path = "/".to_string();
        }
        path
    } else {
        String::new()
    };
    // Authority: [*.]host[:port] — strip brackets for IPv6 comparison later (keep as-is lowercased).
    let mut host_port = authority;
    // Handle wildcard prefix.
    let mut wildcard = false;
    if let Some(stripped) = host_port.strip_prefix("*.") {
        wildcard = true;
        host_port = stripped;
    }
    if host_port.starts_with('[') {
        if let Some(end) = host_port.find(']') {
            let host = host_port[..=end].to_ascii_lowercase();
            parts.host = if wildcard { format!("*.{host}") } else { host };
            let remainder = &host_port[end + 1..];
            if let Some(p_str) = remainder.strip_prefix(':') {
                if let Ok(p) = p_str.parse::<u16>() {
                    parts.port = Some(p);
                }
            }
        } else {
            parts.host = host_port.to_ascii_lowercase();
        }
    } else if let Some(colon) = host_port.rfind(':') {
        let (h, p_str) = (&host_port[..colon], &host_port[colon + 1..]);
        if !p_str.is_empty() && p_str.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(p) = p_str.parse::<u16>() {
                parts.host = if wildcard {
                    format!("*.{}", h.to_ascii_lowercase())
                } else {
                    h.to_ascii_lowercase()
                };
                parts.port = Some(p);
            } else {
                parts.host = host_port.to_ascii_lowercase();
            }
        } else {
            parts.host = host_port.to_ascii_lowercase();
        }
    } else {
        parts.host = if wildcard {
            // Re-add prefix for caller detection; parse_csp_url callers strip it.
            format!("*.{}", host_port.to_ascii_lowercase())
        } else {
            host_port.to_ascii_lowercase()
        };
    }
    parts
}

fn csp_self_matches(url: &CspUrlParts, page: &CspUrlParts) -> bool {
    if url.scheme != page.scheme || url.host != page.host {
        return false;
    }
    let url_port = url.port.or_else(|| default_port_for(&url.scheme));
    let page_port = page.port.or_else(|| default_port_for(&page.scheme));
    url_port == page_port
}

/// Security-relevant response headers, parsed into a single struct.
#[derive(Debug, Clone, Default)]
pub struct SecurityHeaders {
    pub content_security_policy: Option<CspPolicy>,
    /// All enforced CSP policies (multiple `Content-Security-Policy` headers
    /// must ALL allow a resource — last-wins is a bypass).
    pub content_security_policies: Vec<CspPolicy>,
    pub hsts: Option<HstsDirective>,
    pub x_frame_options: Option<String>,
    pub x_content_type_options: Option<String>,
    pub referrer_policy: Option<String>,
}

impl SecurityHeaders {
    /// Extracts security headers from a header map (case-insensitive).
    /// When the map contains a single combined CSP value, it is split into
    /// individual policies; use [`from_headers_multi`] to preserve truly
    /// separate headers.
    pub fn from_headers(headers: &HashMap<String, String>) -> SecurityHeaders {
        let get = |name: &str| -> Option<&String> {
            headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v)
        };

        // Collect every CSP header value; a HashMap can only hold one, but
        // callers that join multiples with '\n' are also handled.
        let mut policies = Vec::new();
        for (k, v) in headers.iter() {
            if k.eq_ignore_ascii_case("content-security-policy") {
                for line in v.split('\n') {
                    for part in line.split(',') {
                        let p = part.trim();
                        if p.is_empty() {
                            continue;
                        }
                        if let Some(policy) = CspPolicy::parse(p, false) {
                            policies.push(policy);
                        }
                    }
                }
            }
        }
        // Also handle report-only separately (never enforced).
        let csp = policies.first().cloned();
        let hsts = get("strict-transport-security").and_then(|v| HstsDirective::parse(v));

        SecurityHeaders {
            content_security_policy: csp,
            content_security_policies: policies,
            hsts,
            x_frame_options: get("x-frame-options").cloned(),
            x_content_type_options: get("x-content-type-options").cloned(),
            referrer_policy: get("referrer-policy").cloned(),
        }
    }

    /// Extracts security headers from a multi-value list, preserving every
    /// `Content-Security-Policy` header as a separate enforced policy.
    pub fn from_headers_multi(headers: &[(String, String)]) -> SecurityHeaders {
        let get = |name: &str| -> Option<String> {
            headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.clone())
        };
        let mut policies = Vec::new();
        for (k, v) in headers {
            if k.eq_ignore_ascii_case("content-security-policy") {
                for line in v.split('\n') {
                    for part in line.split(',') {
                        let p = part.trim();
                        if p.is_empty() {
                            continue;
                        }
                        if let Some(policy) = CspPolicy::parse(p, false) {
                            policies.push(policy);
                        }
                    }
                }
            }
        }
        let csp = policies.first().cloned();
        let hsts = get("strict-transport-security").and_then(|v| HstsDirective::parse(&v));
        SecurityHeaders {
            content_security_policy: csp,
            content_security_policies: policies,
            hsts,
            x_frame_options: get("x-frame-options"),
            x_content_type_options: get("x-content-type-options"),
            referrer_policy: get("referrer-policy"),
        }
    }

    /// True when `url` is allowed by EVERY enforced CSP policy.
    pub fn allows_all(&self, directive: ResourceDirective, url: &str, page_origin: &str) -> bool {
        for policy in &self.content_security_policies {
            if !policy.is_allowed(directive, url, page_origin) {
                return false;
            }
        }
        true
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
                .content_security_policies
                .iter()
                .any(|csp| csp.blocks_framing())
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
    /// Parses an HSTS header. Returns `None` when `max-age` is absent —
    /// a bare `includeSubDomains` must not delete or create a policy.
    pub fn parse(value: &str) -> Option<HstsDirective> {
        let mut max_age: Option<u64> = None;
        let mut include_subdomains = false;
        let mut preload = false;
        for part in value.split(';') {
            let part = part.trim();
            if part.len() >= 8 && part[..8].eq_ignore_ascii_case("max-age=") {
                let rest = part[8..].trim().trim_matches('"');
                // Take up to whitespace/comma.
                let num_str = rest.split([' ', ',', ';']).next().unwrap_or("").trim();
                max_age = Some(num_str.parse().unwrap_or(0));
            } else if part.eq_ignore_ascii_case("includeSubDomains") {
                include_subdomains = true;
            } else if part.eq_ignore_ascii_case("preload") {
                preload = true;
            }
        }
        let max_age_secs = max_age?;
        Some(HstsDirective {
            max_age_secs,
            include_subdomains,
            preload,
        })
    }
}

/// Maximum number of stored HSTS host policies to prevent unbounded memory growth.
pub const MAX_HSTS_ENTRIES: usize = 1024;

/// An in-memory HSTS store (RFC 6797) with expiry.
#[derive(Debug)]
pub struct HstsStore {
    entries: HashMap<String, (HstsDirective, Instant)>,
    /// Whether the store should also upgrade subdomains known via `includeSubDomains`.
    subdomains: HashMap<String, bool>,
    last_pruned: Instant,
}

impl Default for HstsStore {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            subdomains: HashMap::new(),
            last_pruned: Instant::now()
                .checked_sub(Duration::from_secs(60))
                .unwrap_or_else(Instant::now),
        }
    }
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

        // Automatic pruning of expired entries on write.
        if self.entries.len() >= MAX_HSTS_ENTRIES
            || self.last_pruned.elapsed() >= Duration::from_secs(10)
        {
            self.prune();
        }

        // If still at capacity after pruning expired entries, evict the entry closest to expiration.
        if self.entries.len() >= MAX_HSTS_ENTRIES && !self.entries.contains_key(&host) {
            if let Some(oldest_host) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, exp))| *exp)
                .map(|(h, _)| h.clone())
            {
                self.entries.remove(&oldest_host);
                self.subdomains.remove(&oldest_host);
            }
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
        self.last_pruned = now;
        self.entries.retain(|_, (_, expires)| *expires > now);
        let active: std::collections::HashSet<&str> =
            self.entries.keys().map(|s| s.as_str()).collect();
        self.subdomains
            .retain(|host, _| active.contains(host.as_str()));
    }
}

/// Returns true when an HTTP subresource on an HTTPS page must be blocked.
///
/// Both active mixed content (scripts, stylesheets) and passive mixed content
/// (images, media) are blocked to prevent MITM attacks and cookie exfiltration.
pub fn blocks_mixed_content(page_url: &str, resource_url: &str, _active: bool) -> bool {
    let page_is_https = page_url.trim().to_ascii_lowercase().starts_with("https://");
    let resource_is_http = resource_url
        .trim()
        .to_ascii_lowercase()
        .starts_with("http://");
    page_is_https && resource_is_http
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

/// Request credentials mode (W3C Fetch specification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestCredentials {
    /// Exclude credentials (cookies, HTTP basic auth) from the request.
    Omit,
    /// Include credentials only if target is same-origin.
    #[default]
    SameOrigin,
    /// Always include credentials, requiring Access-Control-Allow-Credentials: true on cross-origin.
    Include,
}

impl RequestCredentials {
    pub fn as_str(&self) -> &'static str {
        match self {
            RequestCredentials::Omit => "omit",
            RequestCredentials::SameOrigin => "same-origin",
            RequestCredentials::Include => "include",
        }
    }
}

/// Evaluates CORS response headers using a high-level `RequestCredentials` mode.
pub fn validate_cors_response_with_credentials(
    response_headers: &HashMap<String, String>,
    request_origin: &str,
    target_origin: &str,
    credentials: RequestCredentials,
) -> Result<Vec<String>, CorsError> {
    let sends_credentials = match credentials {
        RequestCredentials::Omit => false,
        RequestCredentials::SameOrigin => request_origin.eq_ignore_ascii_case(target_origin),
        RequestCredentials::Include => true,
    };
    validate_cors_response(response_headers, request_origin, sends_credentials)
}

// ---------------------------------------------------------------------------
// Pure-Rust Cryptographic Hashes (FIPS 180-4): SHA-256 & SHA-384 / SHA-512
// ---------------------------------------------------------------------------

pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut h = [
        0x6a09e667u32,
        0xbb67ae85u32,
        0x3c6ef372u32,
        0xa54ff53au32,
        0x510e527fu32,
        0x9b05688cu32,
        0x1f83d9abu32,
        0x5be0cd19u32,
    ];

    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut h_var = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_var
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h_var = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_var);
    }

    let mut out = [0u8; 32];
    for (i, val) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&val.to_be_bytes());
    }
    out
}

pub fn sha384(data: &[u8]) -> [u8; 48] {
    let full = sha512_internal(data, true);
    let mut out = [0u8; 48];
    out.copy_from_slice(&full[..48]);
    out
}

pub fn sha512(data: &[u8]) -> [u8; 64] {
    sha512_internal(data, false)
}

fn sha512_internal(data: &[u8], is_384: bool) -> [u8; 64] {
    const K512: [u64; 80] = [
        0x428a2f98d728ae22,
        0x7137449123ef65cd,
        0xb5c0fbcfec4d3b2f,
        0xe9b5dba58189dbbc,
        0x3956c25bf348b538,
        0x59f111f1b605d019,
        0x923f82a4af194f9b,
        0xab1c5ed5da6d8118,
        0xd807aa98a3030242,
        0x12835b0145706fbe,
        0x243185be4ee4b28c,
        0x550c7dc3d5ffb4e2,
        0x72be5d74f27b896f,
        0x80deb1fe3b1696b1,
        0x9bdc06a725c71235,
        0xc19bf174cf692694,
        0xe49b69c19ef14ad2,
        0xefbe4786384f25e3,
        0x0fc19dc68b8cd5b5,
        0x240ca1cc77ac9c65,
        0x2de92c6f592b0275,
        0x4a7484aa6ea6e483,
        0x5cb0a9dcbd41fbd4,
        0x76f988da831153b5,
        0x983e5152ee66dfab,
        0xa831c66d2db43210,
        0xb00327c898fb213f,
        0xbf597fc7beef0ee4,
        0xc6e00bf33da88fc2,
        0xd5a79147930aa725,
        0x06ca6351e003826f,
        0x142929670a0e6e70,
        0x27b70a8546d22ffc,
        0x2e1b21385c26c926,
        0x4d2c6dfc5ac42aed,
        0x53380d139d95b3df,
        0x650a73548baf63de,
        0x766a0abb3c77b2a8,
        0x81c2c92e47edaee6,
        0x92722c851482353b,
        0xa2bfe8a14cf10364,
        0xa81a664bbc423001,
        0xc24b8b70d0f89791,
        0xc76c51a30654be30,
        0xd192e819d6ef5218,
        0xd69906245565a910,
        0xf40e35855771202a,
        0x106aa07032bbd1b8,
        0x19a4c116b8d2d0c8,
        0x1e376c085141ab53,
        0x2748774cdf8eeb99,
        0x34b0bcb5e19b48a8,
        0x391c0cb3c5c95a63,
        0x4ed8aa4ae3418acb,
        0x5b9cca4f7763e373,
        0x682e6ff3d6b2b8a3,
        0x748f82ee5defb2fc,
        0x78a5636f43172f60,
        0x84c87814a1f0ab72,
        0x8cc702081a6439ec,
        0x90befffa23631e28,
        0xa4506cebde82bde9,
        0xbef9a3f7b2c67915,
        0xc67178f2e372532b,
        0xca273eceea26619c,
        0xd186b8c721c0c207,
        0xeada7dd6cde0eb1e,
        0xf57d4f7fee6ed178,
        0x06f067aa72176fba,
        0x0a637dc5a2c898a6,
        0x113f9804bef90dae,
        0x1b710b35131c471b,
        0x28db77f523047d84,
        0x32caab7b40c72493,
        0x3c9ebe0a15c9bebc,
        0x431d67c49c100d4c,
        0x4cc5d4becb3e42b6,
        0x597f299cfc657e2a,
        0x5fcb6fab3ad6faec,
        0x6c44198c4a475817,
    ];

    let mut h: [u64; 8] = if is_384 {
        [
            0xcbbb9d5dc1059ed8,
            0x629a292a367cd507,
            0x9159015a3070dd17,
            0x152fecd8f70e5939,
            0x67332667ffc00b31,
            0x8eb44a8768581511,
            0xdb0c2e0d64f98fa7,
            0x47b5481dbefa4fa4,
        ]
    } else {
        [
            0x6a09e667f3bcc908,
            0xbb67ae8584caa73b,
            0x3c6ef372fe94f82b,
            0xa54ff53a5f1d36f1,
            0x510e527fade682d1,
            0x9b05688c2b3e6c1f,
            0x1f83d9abfb41bd6b,
            0x5be0cd19137e2179,
        ]
    };

    let mut msg = data.to_vec();
    let bit_len = (data.len() as u128) * 8;
    msg.push(0x80);
    while (msg.len() % 128) != 112 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(128) {
        let mut w = [0u64; 80];
        for i in 0..16 {
            w[i] = u64::from_be_bytes(chunk[i * 8..(i + 1) * 8].try_into().unwrap());
        }
        for i in 16..80 {
            let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
            let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut h_var = h[7];

        for i in 0..80 {
            let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_var
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K512[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h_var = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_var);
    }

    let mut out = [0u8; 64];
    for (i, val) in h.iter().enumerate() {
        out[i * 8..(i + 1) * 8].copy_from_slice(&val.to_be_bytes());
    }
    out
}

// ---------------------------------------------------------------------------
// Subresource Integrity (SRI) Engine (W3C SRI Specification)
// ---------------------------------------------------------------------------

/// Supported cryptographic hash algorithms for Subresource Integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SriAlgorithm {
    Sha256 = 1,
    Sha384 = 2,
    Sha512 = 3,
}

impl SriAlgorithm {
    pub fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix.to_ascii_lowercase().as_str() {
            "sha256" => Some(SriAlgorithm::Sha256),
            "sha384" => Some(SriAlgorithm::Sha384),
            "sha512" => Some(SriAlgorithm::Sha512),
            _ => None,
        }
    }
}

/// An individual SRI hash expression consisting of algorithm and expected base64 digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SriItem {
    pub algorithm: SriAlgorithm,
    pub digest_base64: String,
}

/// Errors occurring during Subresource Integrity validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SriError {
    NoValidMetadata,
    DigestMismatch {
        algorithm: SriAlgorithm,
        expected: String,
        actual: String,
    },
}

impl std::fmt::Display for SriError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SriError::NoValidMetadata => {
                write!(f, "No valid cryptographic hash found in SRI metadata")
            }
            SriError::DigestMismatch {
                algorithm,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "SRI {algorithm:?} digest mismatch: expected '{expected}', calculated '{actual}'"
                )
            }
        }
    }
}

impl std::error::Error for SriError {}

/// Parses the integrity attribute value into a list of recognized SRI items.
pub fn parse_sri_metadata(integrity_attr: &str) -> Vec<SriItem> {
    let mut items = Vec::new();
    for token in integrity_attr.split_whitespace() {
        if let Some((algo_prefix, digest)) = token.split_once('-') {
            if let Some(algorithm) = SriAlgorithm::from_prefix(algo_prefix) {
                // Remove optional options suffix '?options'
                let clean_digest = digest.split('?').next().unwrap_or(digest).trim();
                if !clean_digest.is_empty() {
                    items.push(SriItem {
                        algorithm,
                        digest_base64: clean_digest.to_string(),
                    });
                }
            }
        }
    }
    items
}

/// Verifies resource content against the given SRI `integrity` attribute.
/// Conforms to W3C Subresource Integrity §3.3.
pub fn verify_subresource_integrity(bytes: &[u8], integrity_attr: &str) -> Result<bool, SriError> {
    let items = parse_sri_metadata(integrity_attr);
    if items.is_empty() {
        return Ok(true); // Empty or unrecognized integrity expression is ignored by spec
    }

    // Select the strongest algorithm present
    let strongest_algo = items.iter().map(|item| item.algorithm).max().unwrap();
    let candidates: Vec<&SriItem> = items
        .iter()
        .filter(|i| i.algorithm == strongest_algo)
        .collect();

    let actual_base64 = match strongest_algo {
        SriAlgorithm::Sha256 => crate::websocket::base64_encode(&sha256(bytes)),
        SriAlgorithm::Sha384 => crate::websocket::base64_encode(&sha384(bytes)),
        SriAlgorithm::Sha512 => crate::websocket::base64_encode(&sha512(bytes)),
    };

    // If any candidate hash matches, the resource is authentic
    for cand in &candidates {
        if cand.digest_base64 == actual_base64 {
            return Ok(true);
        }
    }

    Err(SriError::DigestMismatch {
        algorithm: strongest_algo,
        expected: candidates[0].digest_base64.clone(),
        actual: actual_base64,
    })
}

// ---------------------------------------------------------------------------
// Certificate Pinning (HPKP / SPKI Pinning)
// ---------------------------------------------------------------------------

/// Certificate Pinning management and verification.
#[derive(Debug, Clone, Default)]
pub struct CertificatePinSet {
    /// Maps normalized hostname (e.g., "api.example.com") to set of allowed base64-encoded SPKI SHA-256 pin hashes.
    pub host_pins: HashMap<String, Vec<String>>,
    /// Include subdomains flags per host.
    pub include_subdomains: HashMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinError {
    PinMismatch {
        host: String,
        presented: Vec<String>,
        expected: Vec<String>,
    },
}

impl std::fmt::Display for PinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PinError::PinMismatch {
                host,
                presented,
                expected,
            } => {
                write!(
                    f,
                    "Certificate pinning mismatch for {host}: presented {presented:?}, expected {expected:?}"
                )
            }
        }
    }
}

impl std::error::Error for PinError {}

impl CertificatePinSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_pin(
        &mut self,
        host: &str,
        pin_hash_base64: impl Into<String>,
        include_subdomains: bool,
    ) {
        let h = host.trim().to_ascii_lowercase();
        self.host_pins
            .entry(h.clone())
            .or_default()
            .push(pin_hash_base64.into());
        if include_subdomains {
            self.include_subdomains.insert(h, true);
        }
    }

    pub fn verify_pins(
        &self,
        host: &str,
        presented_spki_hashes: &[String],
    ) -> Result<bool, PinError> {
        let host_lower = host.trim().to_ascii_lowercase();
        // Check exact match or ancestor subdomain match
        let mut matching_host = None;
        if self.host_pins.contains_key(&host_lower) {
            matching_host = Some(host_lower.clone());
        } else {
            let parts: Vec<&str> = host_lower.split('.').collect();
            for i in 1..parts.len() {
                let ancestor = parts[i..].join(".");
                if let Some(true) = self.include_subdomains.get(&ancestor) {
                    matching_host = Some(ancestor);
                    break;
                }
            }
        }

        let Some(target_host) = matching_host else {
            return Ok(true); // No pins configured for this host
        };

        let expected_pins = self.host_pins.get(&target_host).unwrap();
        for presented in presented_spki_hashes {
            if expected_pins.iter().any(|exp| exp == presented) {
                return Ok(true);
            }
        }

        Err(PinError::PinMismatch {
            host: host.to_string(),
            presented: presented_spki_hashes.to_vec(),
            expected: expected_pins.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// Network Proxy Configuration & Resolution
// ---------------------------------------------------------------------------

/// Network proxy configuration supporting HTTP/HTTPS/SOCKS5 proxies and bypass rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProxyConfig {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub all_proxy: Option<String>,
    pub no_proxy: Vec<String>,
}

impl ProxyConfig {
    pub fn from_env() -> Self {
        let http_proxy = std::env::var("HTTP_PROXY")
            .or_else(|_| std::env::var("http_proxy"))
            .ok();
        let https_proxy = std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .ok();
        let all_proxy = std::env::var("ALL_PROXY")
            .or_else(|_| std::env::var("all_proxy"))
            .ok();
        let no_proxy_str = std::env::var("NO_PROXY")
            .or_else(|_| std::env::var("no_proxy"))
            .unwrap_or_default();
        let no_proxy = no_proxy_str
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        Self {
            http_proxy,
            https_proxy,
            all_proxy,
            no_proxy,
        }
    }

    pub fn should_bypass(&self, host: &str) -> bool {
        let h = host.trim().to_ascii_lowercase();
        if h == "localhost" || h == "127.0.0.1" || h == "::1" {
            return true;
        }
        for pattern in &self.no_proxy {
            if pattern == "*" {
                return true;
            }
            if pattern.starts_with('.') {
                if h.ends_with(pattern) || h == &pattern[1..] {
                    return true;
                }
            } else if h == *pattern || h.ends_with(&format!(".{pattern}")) {
                return true;
            }
        }
        false
    }

    pub fn resolve_proxy_for_url(&self, url: &crate::url::Url) -> Option<String> {
        let host = url.host.as_deref()?;
        if self.should_bypass(host) {
            return None;
        }
        let scheme = url.scheme.as_str().to_ascii_lowercase();
        if scheme == "https" {
            self.https_proxy.clone().or_else(|| self.all_proxy.clone())
        } else if scheme == "http" {
            self.http_proxy.clone().or_else(|| self.all_proxy.clone())
        } else {
            self.all_proxy.clone()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Referrer-Policy (W3C Referrer Policy Level 1)
// ─────────────────────────────────────────────────────────────────────────────

/// W3C Referrer Policy directives controlling the `Referer` request header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReferrerPolicy {
    /// Never send the `Referer` header.
    NoReferrer,
    /// Send full URL to equally or more secure destinations (HTTPS->HTTPS, HTTP->HTTP),
    /// but omit when downgrading (HTTPS->HTTP).
    NoReferrerWhenDowngrade,
    /// Only send the origin (scheme, host, port) in all requests.
    Origin,
    /// Send full URL for same-origin; send only origin for cross-origin.
    OriginWhenCrossOrigin,
    /// Send full URL for same-origin requests; omit for cross-origin.
    SameOrigin,
    /// Send origin for secure/same-level destinations; omit on downgrade (HTTPS->HTTP).
    StrictOrigin,
    /// Default modern policy: send full URL for same-origin, origin for cross-origin
    /// (HTTPS->HTTPS), and omit on downgrade (HTTPS->HTTP).
    #[default]
    StrictOriginWhenCrossOrigin,
    /// Send full URL for all requests (even on downgrade).
    UnsafeUrl,
}

impl ReferrerPolicy {
    /// Parses a `Referrer-Policy` header value.
    ///
    /// Per W3C spec, comma-separated tokens are processed left-to-right,
    /// and the last recognized token takes precedence.
    pub fn parse(header: &str) -> Self {
        let mut policy = ReferrerPolicy::default();
        for token in header.split(',') {
            match token.trim().to_ascii_lowercase().as_str() {
                "no-referrer" => policy = ReferrerPolicy::NoReferrer,
                "no-referrer-when-downgrade" => policy = ReferrerPolicy::NoReferrerWhenDowngrade,
                "origin" => policy = ReferrerPolicy::Origin,
                "origin-when-cross-origin" => policy = ReferrerPolicy::OriginWhenCrossOrigin,
                "same-origin" => policy = ReferrerPolicy::SameOrigin,
                "strict-origin" => policy = ReferrerPolicy::StrictOrigin,
                "strict-origin-when-cross-origin" => {
                    policy = ReferrerPolicy::StrictOriginWhenCrossOrigin
                }
                "unsafe-url" => policy = ReferrerPolicy::UnsafeUrl,
                _ => {} // Unknown or deprecated tokens are ignored
            }
        }
        policy
    }
}

/// Computes the `Referer` header value to be sent with a request, following W3C Referrer Policy.
///
/// Strips userinfo (`username`/`password`) and fragment identifiers per RFC 7231 §5.5.2.
/// Returns `None` if the header should be omitted entirely.
pub fn compute_referrer(
    page_url: &crate::url::Url,
    destination_url: &crate::url::Url,
    policy: ReferrerPolicy,
) -> Option<String> {
    // 1. If page is not http or https (e.g. data:, blob:, file:, about:), never send Referer.
    if !page_url.is_http_or_https() {
        return None;
    }
    // 2. If destination is not http or https, never send Referer.
    if !destination_url.is_http_or_https() {
        return None;
    }

    let is_same_origin = page_url
        .scheme
        .eq_ignore_ascii_case(&destination_url.scheme)
        && page_url.host == destination_url.host
        && page_url.port == destination_url.port;

    // TLS downgrade occurs when source is https and target is http.
    let is_downgrade = page_url.scheme.eq_ignore_ascii_case("https")
        && destination_url.scheme.eq_ignore_ascii_case("http");

    match policy {
        ReferrerPolicy::NoReferrer => None,
        ReferrerPolicy::NoReferrerWhenDowngrade => {
            if is_downgrade {
                None
            } else {
                page_url.stripped_for_referrer()
            }
        }
        ReferrerPolicy::SameOrigin => {
            if is_same_origin {
                page_url.stripped_for_referrer()
            } else {
                None
            }
        }
        ReferrerPolicy::Origin => page_url.origin_url(),
        ReferrerPolicy::StrictOrigin => {
            if is_downgrade {
                None
            } else {
                page_url.origin_url()
            }
        }
        ReferrerPolicy::OriginWhenCrossOrigin => {
            if is_same_origin {
                page_url.stripped_for_referrer()
            } else {
                page_url.origin_url()
            }
        }
        ReferrerPolicy::StrictOriginWhenCrossOrigin => {
            if is_same_origin {
                page_url.stripped_for_referrer()
            } else if is_downgrade {
                None
            } else {
                page_url.origin_url()
            }
        }
        ReferrerPolicy::UnsafeUrl => page_url.stripped_for_referrer(),
    }
}

/// Convenience helper to compute Referer with an optional policy string.
pub fn compute_referrer_with_policy_str(
    page_url: &crate::url::Url,
    destination_url: &crate::url::Url,
    policy_str: Option<&str>,
) -> Option<String> {
    let policy = policy_str.map(ReferrerPolicy::parse).unwrap_or_default();
    compute_referrer(page_url, destination_url, policy)
}

/// Convenience helper parsing URL strings to compute Referer.
pub fn compute_referrer_from_str(
    page_url: &str,
    destination_url: &str,
    policy_str: Option<&str>,
) -> Option<String> {
    let p_url = crate::url::Url::parse(page_url).ok()?;
    let d_url = crate::url::Url::parse(destination_url).ok()?;
    compute_referrer_with_policy_str(&p_url, &d_url, policy_str)
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
        // data: requires an explicit `data:` source — 'self' alone denies.
        let policy = CspPolicy::parse("img-src 'self'", false).unwrap();
        assert!(!policy.allows(
            ResourceDirective::Image,
            "data:image/png;base64,AAAA",
            "https://example.com"
        ));
        // Explicit data: source allows.
        let explicit = CspPolicy::parse("img-src 'self' data:", false).unwrap();
        assert!(explicit.allows(
            ResourceDirective::Image,
            "data:image/png;base64,AAAA",
            "https://example.com"
        ));
        // `*` does not allow data: (explicit source required).
        let star = CspPolicy::parse("img-src *", false).unwrap();
        assert!(!star.allows(
            ResourceDirective::Image,
            "data:image/png;base64,AAAA",
            "https://example.com"
        ));
        // blob: similarly requires explicit source.
        assert!(!policy.allows(
            ResourceDirective::Image,
            "blob:https://example.com/uuid",
            "https://example.com"
        ));
        let blob_ok = CspPolicy::parse("img-src 'self' blob:", false).unwrap();
        assert!(blob_ok.allows(
            ResourceDirective::Image,
            "blob:https://example.com/uuid",
            "https://example.com"
        ));
    }

    #[test]
    fn test_csp_port_and_path_matching() {
        let policy = CspPolicy::parse("script-src https://example.com:8443/js/", false).unwrap();
        assert!(policy.allows(
            ResourceDirective::Script,
            "https://example.com:8443/js/app.js",
            "https://example.com"
        ));
        assert!(!policy.allows(
            ResourceDirective::Script,
            "https://example.com:8443/other/app.js",
            "https://example.com"
        ));
        assert!(!policy.allows(
            ResourceDirective::Script,
            "https://example.com:9000/js/app.js",
            "https://example.com"
        ));
        // Bare host matches any scheme but still checks port/path when given.
        let bare = CspPolicy::parse("script-src example.com", false).unwrap();
        assert!(bare.allows(
            ResourceDirective::Script,
            "https://example.com/app.js",
            "https://other.test"
        ));
        assert!(bare.allows(
            ResourceDirective::Script,
            "http://example.com/app.js",
            "https://other.test"
        ));
        // 'self' respects ports.
        let self_policy = CspPolicy::parse("script-src 'self'", false).unwrap();
        assert!(self_policy.allows(
            ResourceDirective::Script,
            "https://example.com/app.js",
            "https://example.com"
        ));
        assert!(!self_policy.allows(
            ResourceDirective::Script,
            "https://example.com:8443/app.js",
            "https://example.com"
        ));
    }

    #[test]
    fn test_hsts_parse_requires_max_age() {
        assert!(HstsDirective::parse("includeSubDomains").is_none());
        assert!(HstsDirective::parse("").is_none());
        assert!(HstsDirective::parse("preload").is_none());
        let d = HstsDirective::parse("max-age=100; includeSubDomains").unwrap();
        assert_eq!(d.max_age_secs, 100);
    }

    #[test]
    fn test_multiple_csp_headers_enforce_all() {
        let headers = vec![
            (
                "Content-Security-Policy".to_string(),
                "script-src 'self'".to_string(),
            ),
            (
                "Content-Security-Policy".to_string(),
                "script-src https://cdn.example.com".to_string(),
            ),
        ];
        let parsed = SecurityHeaders::from_headers_multi(&headers);
        assert_eq!(parsed.content_security_policies.len(), 2);
        // Must satisfy BOTH policies.
        assert!(!parsed.allows_all(
            ResourceDirective::Script,
            "https://cdn.example.com/app.js",
            "https://example.com"
        ));
        assert!(!parsed.allows_all(
            ResourceDirective::Script,
            "https://example.com/app.js",
            "https://example.com"
        ));
        let compatible = vec![
            (
                "Content-Security-Policy".to_string(),
                "script-src 'self' https://cdn.example.com".to_string(),
            ),
            (
                "Content-Security-Policy".to_string(),
                "script-src https://cdn.example.com 'self'".to_string(),
            ),
        ];
        let parsed2 = SecurityHeaders::from_headers_multi(&compatible);
        assert!(parsed2.allows_all(
            ResourceDirective::Script,
            "https://cdn.example.com/app.js",
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
    fn test_hsts_store_automatic_pruning_and_cap() {
        let mut store = HstsStore::new();
        // Insert an entry that will immediately expire (duration 0 simulated or very short)
        store.entries.insert(
            "expired.test".to_string(),
            (
                HstsDirective {
                    max_age_secs: 1,
                    include_subdomains: false,
                    preload: false,
                },
                Instant::now() - Duration::from_secs(10),
            ),
        );
        store.subdomains.insert("expired.test".to_string(), false);
        assert_eq!(store.len(), 1);

        // Recording a new entry triggers auto-prune on modulo/capacity check.
        store.record(
            "fresh.test",
            HstsDirective {
                max_age_secs: 300,
                include_subdomains: true,
                preload: false,
            },
        );
        // The expired entry was pruned automatically!
        assert_eq!(store.len(), 1);
        assert!(store.policy_for("fresh.test").is_some());
        assert!(store.policy_for("expired.test").is_none());

        // Test capacity cap
        for i in 0..(MAX_HSTS_ENTRIES + 10) {
            store.record(
                &format!("host{i}.test"),
                HstsDirective {
                    max_age_secs: 3600,
                    include_subdomains: false,
                    preload: false,
                },
            );
        }
        assert!(store.len() <= MAX_HSTS_ENTRIES);
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
        // Passive mixed content on an HTTPS page is also blocked
        assert!(blocks_mixed_content(
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
    fn test_csp_none_precedence() {
        // Even if permissive source `*` or domain appears before/after `'none'`,
        // `'none'` must strictly deny all requests.
        let policy1 = CspPolicy::parse("script-src * 'none'", false).unwrap();
        assert!(!policy1.allows(
            ResourceDirective::Script,
            "https://example.com/app.js",
            "https://example.com"
        ));

        let policy2 = CspPolicy::parse("script-src 'none' *", false).unwrap();
        assert!(!policy2.allows(
            ResourceDirective::Script,
            "https://example.com/app.js",
            "https://example.com"
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

    #[test]
    fn test_cryptographic_hashes_and_sri() {
        let msg = b"hello world";
        // Standard SHA-256 for "hello world" = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        let d256 = sha256(msg);
        let hex256 = d256.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(
            hex256,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );

        // Standard SHA-384 for "hello world"
        let d384 = sha384(msg);
        let hex384 = d384.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(
            hex384,
            "fdbd8e75a67f29f701a4e040385e2e23986303ea10239211af907fcbb83578b3e417cb71ce646efd0819dd8c088de1bd"
        );

        // Standard SHA-512 for "hello world"
        let d512 = sha512(msg);
        let hex512 = d512.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(
            hex512,
            "309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f"
        );

        // Test SRI parsing and verification
        let integrity = format!(
            "sha256-{} sha384-{}",
            crate::websocket::base64_encode(&d256),
            crate::websocket::base64_encode(&d384)
        );
        let parsed = parse_sri_metadata(&integrity);
        assert_eq!(parsed.len(), 2);

        // Valid integrity verification uses strongest algorithm (sha384)
        assert!(verify_subresource_integrity(msg, &integrity).unwrap());

        // Corrupted content triggers DigestMismatch
        let tampered = b"hello malicious world";
        let err = verify_subresource_integrity(tampered, &integrity).unwrap_err();
        assert!(matches!(err, SriError::DigestMismatch { .. }));
    }

    #[test]
    fn test_certificate_pinning() {
        let mut pin_set = CertificatePinSet::new();
        let target_pin = "pin-base64-hash-example";
        pin_set.add_pin("api.mango.dev", target_pin, true);

        // Matching pin succeeds
        let presented = vec![target_pin.to_string()];
        assert!(pin_set.verify_pins("api.mango.dev", &presented).unwrap());

        // Subdomain succeeds because include_subdomains was true
        assert!(pin_set.verify_pins("v1.api.mango.dev", &presented).unwrap());

        // Mismatched pin fails
        let fake = vec!["attacker-pin-hash".to_string()];
        let err = pin_set.verify_pins("api.mango.dev", &fake).unwrap_err();
        assert!(matches!(err, PinError::PinMismatch { .. }));

        // Unpinned host allows any cert
        assert!(pin_set.verify_pins("unpinned.org", &fake).unwrap());
    }

    #[test]
    fn test_proxy_configuration_and_bypass() {
        let mut proxy = ProxyConfig::default();
        proxy.http_proxy = Some("http://proxy.corp.com:8080".to_string());
        proxy.https_proxy = Some("http://secure-proxy.corp.com:8443".to_string());
        proxy.no_proxy = vec![
            "localhost".to_string(),
            ".internal.corp".to_string(),
            "127.0.0.1".to_string(),
        ];

        // Bypass rules
        assert!(proxy.should_bypass("localhost"));
        assert!(proxy.should_bypass("service.internal.corp"));
        assert!(!proxy.should_bypass("external.com"));

        // Proxy resolution
        let u1 = crate::url::Url::parse("http://external.com/api").unwrap();
        assert_eq!(
            proxy.resolve_proxy_for_url(&u1),
            Some("http://proxy.corp.com:8080".to_string())
        );

        let u2 = crate::url::Url::parse("https://external.com/secure").unwrap();
        assert_eq!(
            proxy.resolve_proxy_for_url(&u2),
            Some("http://secure-proxy.corp.com:8443".to_string())
        );

        let u3 = crate::url::Url::parse("https://service.internal.corp/secret").unwrap();
        assert_eq!(proxy.resolve_proxy_for_url(&u3), None);
    }

    #[test]
    fn test_referrer_policy_parsing_and_computation() {
        let page_https = crate::url::Url::parse(
            "https://user:pass@example.com:443/dir/page.html?token=123#frag",
        )
        .unwrap();
        let same_dest = crate::url::Url::parse("https://example.com/assets/style.css").unwrap();
        let cross_https = crate::url::Url::parse("https://other.com/image.png").unwrap();
        let cross_http = crate::url::Url::parse("http://other.com/script.js").unwrap();

        // 1. NoReferrer
        assert_eq!(
            compute_referrer(&page_https, &same_dest, ReferrerPolicy::NoReferrer),
            None
        );
        assert_eq!(
            compute_referrer(&page_https, &cross_https, ReferrerPolicy::NoReferrer),
            None
        );

        // 2. StrictOriginWhenCrossOrigin (default)
        // Same-origin: stripped full URL (no userinfo, no frag)
        assert_eq!(
            compute_referrer(
                &page_https,
                &same_dest,
                ReferrerPolicy::StrictOriginWhenCrossOrigin
            ),
            Some("https://example.com/dir/page.html?token=123".to_string())
        );
        // Cross-origin HTTPS: origin only
        assert_eq!(
            compute_referrer(
                &page_https,
                &cross_https,
                ReferrerPolicy::StrictOriginWhenCrossOrigin
            ),
            Some("https://example.com/".to_string())
        );
        // Downgrade HTTPS -> HTTP: None
        assert_eq!(
            compute_referrer(
                &page_https,
                &cross_http,
                ReferrerPolicy::StrictOriginWhenCrossOrigin
            ),
            None
        );

        // 3. SameOrigin
        assert_eq!(
            compute_referrer(&page_https, &same_dest, ReferrerPolicy::SameOrigin),
            Some("https://example.com/dir/page.html?token=123".to_string())
        );
        assert_eq!(
            compute_referrer(&page_https, &cross_https, ReferrerPolicy::SameOrigin),
            None
        );

        // 4. Origin
        assert_eq!(
            compute_referrer(&page_https, &same_dest, ReferrerPolicy::Origin),
            Some("https://example.com/".to_string())
        );
        assert_eq!(
            compute_referrer(&page_https, &cross_http, ReferrerPolicy::Origin),
            Some("https://example.com/".to_string())
        );

        // 5. StrictOrigin
        assert_eq!(
            compute_referrer(&page_https, &cross_https, ReferrerPolicy::StrictOrigin),
            Some("https://example.com/".to_string())
        );
        assert_eq!(
            compute_referrer(&page_https, &cross_http, ReferrerPolicy::StrictOrigin),
            None
        );

        // 6. UnsafeUrl
        assert_eq!(
            compute_referrer(&page_https, &cross_http, ReferrerPolicy::UnsafeUrl),
            Some("https://example.com/dir/page.html?token=123".to_string())
        );

        // 7. Non-HTTP(S) scheme source (e.g. data:, blob:, file:)
        let page_data = crate::url::Url::parse("data:text/html,test").unwrap();
        assert_eq!(
            compute_referrer(&page_data, &same_dest, ReferrerPolicy::UnsafeUrl),
            None
        );

        // 8. Multiple tokens comma-separated (last valid wins)
        let parsed = ReferrerPolicy::parse("no-referrer, unsafe-url, strict-origin");
        assert_eq!(parsed, ReferrerPolicy::StrictOrigin);
    }
}
