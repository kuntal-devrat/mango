//! RFC 6265 and RFC 6265bis Cookie storage with disk persistence.
//!
//! Owns the browser's cookie jar, storage, eviction, public suffix checking,
//! and tab-separated profile persistence.

pub mod cookie;
pub mod date;
pub mod jar;
pub mod public_suffix;
pub mod same_site;

#[cfg(test)]
mod tests;

pub use cookie::{Cookie, decode_field, encode_field, now_secs};
pub use date::parse_http_date;
pub use jar::{
    CookieJar, MAX_COOKIE_BYTES, MAX_COOKIES_PER_HOST, MAX_COOKIES_TOTAL, MAX_PROFILE_READ_BYTES,
    cookie_domain_matches, default_cookie_path, path_matches,
};
pub use public_suffix::{
    get_registrable_domain, is_ip_address, is_public_suffix, is_same_site_hosts,
};
pub use same_site::{CookiePriority, SameSite};
