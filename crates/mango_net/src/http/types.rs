//! HTTP client, request/response models, and redirect handling.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use crate::url::Url;

/// Errors that can occur during network transport or HTTP communication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkError {
    DnsResolutionFailed(String),
    ConnectionFailed(String),
    TlsError(String),
    Timeout,
    HttpError(u16, String),
    TooManyRedirects,
    InvalidUrl(String),
    IoError(String),
    Other(String),
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkError::DnsResolutionFailed(msg) => write!(f, "DNS resolution failed: {msg}"),
            NetworkError::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            NetworkError::TlsError(msg) => write!(f, "TLS handshake failed: {msg}"),
            NetworkError::Timeout => write!(f, "Network request timed out"),
            NetworkError::HttpError(code, text) => write!(f, "HTTP error {code}: {text}"),
            NetworkError::TooManyRedirects => write!(f, "Too many HTTP redirects (infinite loop)"),
            NetworkError::InvalidUrl(msg) => write!(f, "Invalid URL: {msg}"),
            NetworkError::IoError(msg) => write!(f, "I/O error: {msg}"),
            NetworkError::Other(msg) => write!(f, "Network error: {msg}"),
        }
    }
}

impl std::error::Error for NetworkError {}

/// Inserts an HTTP response header, combining duplicate headers with comma-separation per RFC 9110 §5.3.
pub fn insert_response_header(map: &mut HashMap<String, String>, name: &str, value: &str) {
    let key = name.to_ascii_lowercase();
    map.entry(key)
        .and_modify(|existing| {
            existing.push_str(", ");
            existing.push_str(value);
        })
        .or_insert_with(|| value.to_string());
}

/// Standard HTTP request methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
}

/// An HTTP request to be sent over the wire.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub url: Url,
    pub method: HttpMethod,
    pub headers: HashMap<String, String>,
    pub timeout: Duration,
}

impl HttpRequest {
    pub fn new(url: Url) -> Self {
        let mut headers = HashMap::new();
        headers.insert(
            "User-Agent".to_string(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".to_string(),
        );
        headers.insert(
            "Accept".to_string(),
            "text/html,application/xhtml+xml,text/css,image/*;q=0.9,*/*;q=0.8".to_string(),
        );
        headers.insert(
            "Accept-Encoding".to_string(),
            "gzip, deflate, br".to_string(),
        );

        Self {
            url,
            method: HttpMethod::Get,
            headers,
            timeout: Duration::from_secs(30),
        }
    }
}

/// An HTTP response received from a web server.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Final URL after following any redirects.
    pub final_url: Url,
    /// HTTP status code (e.g. 200, 301, 404).
    pub status: u16,
    /// HTTP status text (e.g. "OK", "Moved Permanently", "Not Found").
    pub status_text: String,
    /// Response headers, keyed in lowercase.
    pub headers: HashMap<String, String>,
    /// HTTP protocol version negotiated for this response.
    pub version: crate::http2::HttpVersion,
    /// Response body bytes (automatically decompressed from gzip if applicable).
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Returns the body as a UTF-8 string, with lossy replacement if invalid UTF-8.
    pub fn body_as_string(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Returns the MIME content-type from headers (e.g. "text/html", "text/css").
    pub fn content_type(&self) -> Option<&str> {
        let full_type = self.headers.get("content-type")?;
        Some(full_type.split(';').next().unwrap_or("").trim())
    }

    /// Returns `true` if the HTTP status indicates success (2xx).
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Returns `true` if the HTTP status is a redirect (301, 302, 303, 307, 308).
    pub fn is_redirect(&self) -> bool {
        matches!(self.status, 301 | 302 | 303 | 307 | 308)
    }

    /// Returns the redirect `Location` header, if present.
    pub fn location(&self) -> Option<&str> {
        self.headers.get("location").map(|s| s.as_str())
    }
}

// Cookie storage lives in its own module so that the jar can grow expiry,
// SameSite, and on-disk persistence without bloating the HTTP client.
pub use crate::cookies::{Cookie, CookieJar, SameSite};

/// Maximum response body bytes (50MB DoS cap).
pub const MAX_RESPONSE_BODY_BYTES: usize = 50 * 1024 * 1024;

/// Reads a ureq body with a 50MB cap, mapping limit errors to [`NetworkError`].
pub(crate) fn read_body_capped(body: ureq::Body) -> Result<Vec<u8>, NetworkError> {
    body.into_with_config()
        .limit(MAX_RESPONSE_BODY_BYTES as u64)
        .read_to_vec()
        .map_err(|e| match &e {
            ureq::Error::BodyExceedsLimit(limit) => NetworkError::Other(format!(
                "Response body exceeds {limit} byte cap (DoS protection)"
            )),
            ureq::Error::Timeout(_) => NetworkError::Timeout,
            ureq::Error::HostNotFound => {
                NetworkError::DnsResolutionFailed("Host not found".to_string())
            }
            ureq::Error::ConnectionFailed => {
                NetworkError::ConnectionFailed("Connection failed".to_string())
            }
            ureq::Error::Tls(msg) => NetworkError::TlsError(msg.to_string()),
            ureq::Error::StatusCode(code) => NetworkError::HttpError(*code, format!("HTTP {code}")),
            other => NetworkError::IoError(format!("Failed to read response body: {other}")),
        })
}
/// Reads a ureq body in chunks up to a 50MB cap, passing each chunk to `on_chunk`.
pub fn read_body_streaming<F: FnMut(&[u8]) -> bool>(
    body: ureq::Body,
    mut on_chunk: F,
) -> Result<Vec<u8>, NetworkError> {
    use std::io::Read;
    let mut reader = body
        .into_with_config()
        .limit(MAX_RESPONSE_BODY_BYTES as u64)
        .reader();
    let mut accumulated = Vec::new();
    let mut buf = [0u8; 32768]; // 32KB chunks
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let chunk = &buf[..n];
                if accumulated.len().saturating_add(n) > MAX_RESPONSE_BODY_BYTES {
                    return Err(NetworkError::IoError(format!(
                        "Response body exceeded maximum limit of {} bytes",
                        MAX_RESPONSE_BODY_BYTES
                    )));
                }
                accumulated.extend_from_slice(chunk);
                if !on_chunk(chunk) {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(NetworkError::IoError(format!("Streaming read error: {e}"))),
        }
    }
    Ok(accumulated)
}

pub(crate) fn map_ureq_error(e: ureq::Error, url: &Url) -> NetworkError {
    match e {
        ureq::Error::Timeout(_) => NetworkError::Timeout,
        ureq::Error::HostNotFound => NetworkError::DnsResolutionFailed(format!(
            "Host not found: {}",
            url.host.as_deref().unwrap_or("")
        )),
        ureq::Error::ConnectionFailed => {
            NetworkError::ConnectionFailed("Connection failed".to_string())
        }
        ureq::Error::Tls(msg) => NetworkError::TlsError(msg.to_string()),
        ureq::Error::StatusCode(code) => NetworkError::HttpError(code, format!("HTTP {code}")),
        ureq::Error::BodyExceedsLimit(limit) => NetworkError::Other(format!(
            "Response body exceeds {limit} byte cap (DoS protection)"
        )),
        other => NetworkError::Other(other.to_string()),
    }
}

pub(crate) fn create_pooled_agent(timeout: Duration) -> std::sync::Arc<ureq::Agent> {
    let config = ureq::config::Config::builder()
        .http_status_as_error(false)
        .no_delay(true)
        .max_idle_connections(256)
        .max_idle_connections_per_host(32)
        .max_idle_age(Duration::from_secs(60))
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_resolve(Some(Duration::from_secs(30)))
        .build();
    std::sync::Arc::new(ureq::Agent::new_with_config(config))
}
