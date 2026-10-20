//! Content Security Policy (CSP Level 3) implementation conforming to W3C specifications.
//!
//! Enforces restrictions on executable and renderable resources (scripts, styles,
//! images, frames, connect, etc.) loaded by Mango.

use super::crypto::{sha256, sha384, sha512};
use super::sri::SriAlgorithm;

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
    pub fn allows_hash(
        &self,
        directive: ResourceDirective,
        algo: SriAlgorithm,
        hash_bytes: &[u8],
    ) -> bool {
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

pub(crate) fn split_url(url: &str) -> (String, String) {
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
