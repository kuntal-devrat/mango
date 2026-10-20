//! HTTP Strict Transport Security (HSTS - RFC 6797) and Security Headers.
//!
//! Enforces TLS-only communication for declared hosts and handles mixed-content blocking.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::csp::{CspPolicy, ResourceDirective, split_url};

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
    pub(crate) entries: HashMap<String, (HstsDirective, Instant)>,
    /// Whether the store should also upgrade subdomains known via `includeSubDomains`.
    pub(crate) subdomains: HashMap<String, bool>,
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
