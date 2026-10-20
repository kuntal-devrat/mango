//! Cross-Origin Resource Sharing (CORS) and Same-Origin Policy (SOP).
//!
//! Implements origin parsing, same-origin checks (RFC 6454), simple request validation,
//! preflight checks, and CORS response validation conforming to W3C Fetch specification.

use std::collections::HashMap;
use std::time::Duration;

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
