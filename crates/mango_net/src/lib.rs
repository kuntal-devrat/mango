//! # mango_net
//!
//! Networking stack: HTTP client, TLS, DNS resolution, caching, URL parsing,
//! and web resource loading for the Mango browser engine.
//!
//! Fully pure-Rust with zero OpenSSL or external C dependencies.

pub mod cache;
pub mod cookies;
pub mod dns;
pub mod encoding;
pub mod http;
pub mod http2;
pub mod pipeline;
pub mod resource_loader;
pub mod security;
pub mod tls;
pub mod url;

pub use cache::{CachedResource, ResourceCache};
pub use cookies::{Cookie, CookieJar, SameSite};
pub use dns::{global_dns_cache, DnsCache};
pub use encoding::{decode_html_bytes, detect_encoding, Encoding};
pub use http::{HttpClient, HttpMethod, HttpRequest, HttpResponse, NetworkError};
pub use http2::{FrameHeader, FrameType, Http2Frame, Http2Session, Http2Stream, HttpVersion};
pub use pipeline::{
    cache_stage, decode_stage, decompress_stage, http_request_stage, resolve_dns_stage,
    tls_handshake_stage, DecodedResource, ResourcePipeline, TlsConnectionInfo,
};
pub use resource_loader::{FetchedDocument, ResourceLoader};
pub use security::{CspPolicy, HstsStore, SecurityHeaders};
pub use tls::tls_backend_info;
pub use url::{Url, UrlError};
