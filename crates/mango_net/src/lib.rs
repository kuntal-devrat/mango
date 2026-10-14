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
pub mod websocket;

pub use cache::{CachedResource, ResourceCache};
pub use cookies::{Cookie, CookieJar, SameSite};
pub use dns::{DnsCache, global_dns_cache};
pub use encoding::{Encoding, decode_html_bytes, detect_encoding};
pub use http::{HttpClient, HttpMethod, HttpRequest, HttpResponse, NetworkError};
pub use http2::{
    AltSvcCache, AltSvcRecord, FrameHeader, FrameType, Http2ConnectionPool, Http2Frame,
    Http2Session, Http2Stream, Http3Frame, HttpVersion, decode_quic_varint, encode_quic_varint,
};
pub use pipeline::{
    DecodedResource, ResourcePipeline, TlsConnectionInfo, cache_stage, decode_stage,
    decompress_stage, http_request_stage, resolve_dns_stage, tls_handshake_stage,
};
pub use resource_loader::{
    FetchPriority, FetchedDocument, PreloadItem, PreloadKind, PreloadScanResult, ResourceLoader,
};
pub use security::{
    CertificatePinSet, CorsError, CspPolicy, HstsStore, Origin, PinError, ProxyConfig,
    SecurityHeaders, SriAlgorithm, SriError, SriItem, is_cors_simple_request, is_same_origin,
    parse_sri_metadata, sha256, sha384, sha512, validate_cors_preflight, validate_cors_response,
    verify_subresource_integrity,
};
pub use tls::tls_backend_info;
pub use url::{Url, UrlError};
pub use websocket::{WebSocketEvent, WebSocketManager};

