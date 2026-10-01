//! TLS (Transport Layer Security) configuration helpers.
//!
//! Mango uses pure-Rust `rustls` with `webpki-roots` for secure,
//! zero-C-dependency HTTPS communication across Windows, Linux, and macOS.
//! Actual TLS connections are established by `ureq` with its `rustls` feature;
//! this module exposes helpers for querying TLS metadata, validating certificate
//! hostname constraints, and checking whether a URL requires an upgrade.

use crate::url::Url;

// ── TLS backend metadata ─────────────────────────────────────────────────────

/// Returns a human-readable description of the active TLS backend.
pub fn tls_backend_info() -> &'static str {
    "rustls (pure-Rust TLS 1.2/1.3 with WebPKI trusted root certificates)"
}

// ── HSTS helpers ─────────────────────────────────────────────────────────────

/// Returns `true` if the given URL must be fetched over HTTPS.
///
/// This covers:
/// - URLs that already use the `https:` scheme.
/// - Well-known domains on the HSTS preload list hardcoded below.
///
/// In a full implementation this would consult a persisted HSTS store updated
/// from `Strict-Transport-Security` response headers and a bundled preload list.
/// For now the preload list covers the most common sites Mango targets.
pub fn requires_https(url: &Url) -> bool {
    if url.scheme == "https" {
        return true;
    }
    // Hardcoded HSTS preload entries for common hosts.
    let host = url
        .host
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    HSTS_PRELOAD_HOSTS
        .iter()
        .any(|&preloaded| host == preloaded || host.ends_with(&format!(".{preloaded}")))
}

/// Attempts to upgrade an `http://` URL to `https://` for HSTS-preloaded hosts.
///
/// Returns `Some(upgraded_url)` when the host is on the preload list and the
/// scheme was `http`, otherwise `None`.
pub fn maybe_upgrade_to_https(url: &Url) -> Option<Url> {
    if url.scheme != "http" {
        return None;
    }
    if requires_https(url) {
        let upgraded = url.as_str().replacen("http://", "https://", 1);
        Url::parse(&upgraded).ok()
    } else {
        None
    }
}

// ── Certificate / hostname helpers ───────────────────────────────────────────

/// Returns `true` if `hostname` looks like a valid DNS name or IPv4/IPv6
/// address that rustls would accept as a TLS server name.
///
/// This is a lightweight syntactic check — actual certificate validation is
/// handled by rustls/webpki inside `ureq`.
pub fn is_valid_tls_hostname(hostname: &str) -> bool {
    if hostname.is_empty() || hostname.len() > 253 {
        return false;
    }
    // IPv6 literals are always valid as server names.
    if hostname.starts_with('[') && hostname.ends_with(']') {
        return true;
    }
    // Each DNS label must be 1–63 chars, ASCII alphanumeric or hyphen,
    // and must not start or end with a hyphen.
    hostname.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '*')
            && !label.starts_with('-')
            && !label.ends_with('-')
    })
}

/// Checks whether a `Set-Cookie` header's `Secure` flag is consistent with
/// the scheme of `url`.  Returns `false` (and logs a warning) if a `Secure`
/// cookie is being set over plain HTTP — a configuration error on the server.
pub fn validate_secure_cookie(url: &Url, cookie_is_secure: bool) -> bool {
    if cookie_is_secure && url.scheme == "http" {
        log::warn!(
            "[tls] Server at {} tried to set a Secure cookie over HTTP — ignoring.",
            url.host.as_deref().unwrap_or("(unknown)")
        );
        return false;
    }
    true
}

// ── Mixed-content check ───────────────────────────────────────────────────────

/// Returns `true` if loading `resource_url` from a page at `page_url` would
/// constitute a mixed-content violation (HTTPS page → HTTP resource).
///
/// Mango blocks mixed content by default to avoid leaking data over plain HTTP.
pub fn is_mixed_content(page_url: &Url, resource_url: &Url) -> bool {
    page_url.scheme == "https" && resource_url.scheme == "http"
}

// ── HSTS preload list (subset) ────────────────────────────────────────────────

/// A minimal hardcoded HSTS preload list covering Mango's primary target sites.
///
/// Source: <https://hstspreload.org> — entries included verbatim under the
/// site's CC0 / public-domain dedication.
const HSTS_PRELOAD_HOSTS: &[&str] = &[
    "google.com",
    "youtube.com",
    "github.com",
    "wikipedia.org",
    "duckduckgo.com",
    "twitter.com",
    "x.com",
    "facebook.com",
    "reddit.com",
    "stackoverflow.com",
    "mozilla.org",
    "rust-lang.org",
    "crates.io",
    "docs.rs",
    "cloudflare.com",
    "amazon.com",
    "apple.com",
    "microsoft.com",
    "bing.com",
    "netflix.com",
    "instagram.com",
    "linkedin.com",
    "nytimes.com",
    "bbc.com",
    "bbc.co.uk",
];

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::url::Url;

    #[test]
    fn test_tls_info_contains_rustls() {
        assert!(tls_backend_info().contains("rustls"));
    }

    #[test]
    fn test_requires_https_already_https() {
        let url = Url::parse("https://example.com/").unwrap();
        assert!(requires_https(&url));
    }

    #[test]
    fn test_requires_https_preloaded_host() {
        let url = Url::parse("http://github.com/").unwrap();
        assert!(requires_https(&url));
    }

    #[test]
    fn test_requires_https_unknown_host() {
        let url = Url::parse("http://myprivateserver.local/").unwrap();
        assert!(!requires_https(&url));
    }

    #[test]
    fn test_maybe_upgrade_to_https() {
        let url = Url::parse("http://github.com/rust-lang/rust").unwrap();
        let upgraded = maybe_upgrade_to_https(&url).expect("should upgrade");
        assert_eq!(upgraded.scheme, "https");
        assert!(upgraded.as_str().contains("github.com"));
    }

    #[test]
    fn test_maybe_upgrade_already_https() {
        let url = Url::parse("https://github.com/").unwrap();
        assert!(maybe_upgrade_to_https(&url).is_none());
    }

    #[test]
    fn test_is_valid_tls_hostname_valid() {
        assert!(is_valid_tls_hostname("example.com"));
        assert!(is_valid_tls_hostname("sub.example.com"));
        assert!(is_valid_tls_hostname("*.example.com"));
        assert!(is_valid_tls_hostname("[::1]"));
    }

    #[test]
    fn test_is_valid_tls_hostname_invalid() {
        assert!(!is_valid_tls_hostname(""));
        assert!(!is_valid_tls_hostname("-bad.com"));
        assert!(!is_valid_tls_hostname("bad-.com"));
    }

    #[test]
    fn test_is_mixed_content() {
        let https_page = Url::parse("https://secure.example.com/").unwrap();
        let http_res = Url::parse("http://cdn.example.com/img.png").unwrap();
        let https_res = Url::parse("https://cdn.example.com/img.png").unwrap();
        assert!(is_mixed_content(&https_page, &http_res));
        assert!(!is_mixed_content(&https_page, &https_res));
    }

    #[test]
    fn test_subdomain_hsts() {
        let url = Url::parse("http://en.wikipedia.org/").unwrap();
        assert!(requires_https(&url));
    }
}
