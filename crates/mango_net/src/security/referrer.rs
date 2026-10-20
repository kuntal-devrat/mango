//! Referrer-Policy implementation conforming to W3C Referrer Policy Level 1.
//!
//! Governs the `Referer` request header generation for subresource and navigation requests.

use crate::url::Url;

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
    page_url: &Url,
    destination_url: &Url,
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
    page_url: &Url,
    destination_url: &Url,
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
    let p_url = Url::parse(page_url).ok()?;
    let d_url = Url::parse(destination_url).ok()?;
    compute_referrer_with_policy_str(&p_url, &d_url, policy_str)
}
