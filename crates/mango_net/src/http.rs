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
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36".to_string(),
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
            timeout: Duration::from_secs(10),
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
    /// HTTP protocol version negotiated for this response (GAP-014).
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

fn create_pooled_agent(_timeout: Duration) -> std::sync::Arc<ureq::Agent> {
    let config = ureq::config::Config::builder()
        .http_status_as_error(false)
        .build();
    std::sync::Arc::new(ureq::Agent::new_with_config(config))
}

/// A high-performance, pure-Rust HTTP client with automatic cookie management and connection pooling.
#[derive(Clone)]
pub struct HttpClient {
    pub user_agent: String,
    pub timeout: Duration,
    pub cookie_jar: std::sync::Arc<std::sync::Mutex<CookieJar>>,
    pub agent: std::sync::Arc<ureq::Agent>,
}

impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpClient")
            .field("user_agent", &self.user_agent)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient {
    /// Creates a new HTTP client with default settings and empty cookie jar.
    pub fn new() -> Self {
        let timeout = Duration::from_secs(10);
        Self {
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36".to_string(),
            timeout,
            cookie_jar: std::sync::Arc::new(std::sync::Mutex::new(CookieJar::new())),
            agent: create_pooled_agent(timeout),
        }
    }

    /// Creates an HTTP client sharing an existing cookie jar.
    pub fn with_cookie_jar(cookie_jar: std::sync::Arc<std::sync::Mutex<CookieJar>>) -> Self {
        let timeout = Duration::from_secs(10);
        Self {
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36".to_string(),
            timeout,
            cookie_jar,
            agent: create_pooled_agent(timeout),
        }
    }

    /// Clears all cookies in this client's jar.
    pub fn clear_cookies(&self) {
        if let Ok(mut jar) = self.cookie_jar.lock() {
            jar.clear();
        }
    }

    /// Returns matching cookies for the given URL.
    pub fn cookies_for_url(&self, url: &Url) -> Vec<Cookie> {
        self.cookie_jar
            .lock()
            .map(|jar| jar.get_cookies(url))
            .unwrap_or_default()
    }

    /// Executes a single HTTP GET request without following redirects.
    pub fn get_single(&self, url: &Url) -> Result<HttpResponse, NetworkError> {
        let url_str = url.as_str();

        let mut req = self.agent
            .get(&url_str)
            .header("User-Agent", &self.user_agent)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("sec-ch-ua", "\"Chromium\";v=\"128\", \"Not;A=Brand\";v=\"24\", \"Google Chrome\";v=\"128\"")
            .header("sec-ch-ua-mobile", "?0")
            .header("sec-ch-ua-platform", "\"Windows\"")
            .header("upgrade-insecure-requests", "1");

        if let Ok(jar) = self.cookie_jar.lock()
            && let Some(cookie_hdr) = jar.get_cookie_header(url)
        {
            req = req.header("Cookie", &cookie_hdr);
        }

        let response = req.call().map_err(|e| match e {
            ureq::Error::Timeout(_) => NetworkError::Timeout,
            ureq::Error::HostNotFound => {
                NetworkError::DnsResolutionFailed(format!("Host not found: {}", url.host.as_deref().unwrap_or("")))
            }
            ureq::Error::ConnectionFailed => NetworkError::ConnectionFailed("Connection failed".to_string()),
            ureq::Error::Tls(msg) => NetworkError::TlsError(msg.to_string()),
            ureq::Error::StatusCode(code) => {
                NetworkError::HttpError(code, format!("HTTP {code}"))
            }
            other => NetworkError::Other(other.to_string()),
        })?;

        // Extract Set-Cookie headers into the cookie jar
        if let Ok(mut jar) = self.cookie_jar.lock() {
            for (name, val) in response.headers() {
                if name.as_str().eq_ignore_ascii_case("set-cookie")
                    && let Ok(v_str) = val.to_str()
                    && let Some(cookie) = CookieJar::parse_set_cookie(v_str, url)
                {
                    jar.store_cookie(cookie);
                }
            }
        }

        let status = response.status().as_u16();
        let status_text = response.status().canonical_reason().unwrap_or("").to_string();

        let mut headers = HashMap::new();
        for (name, val) in response.headers() {
            if let Ok(v_str) = val.to_str() {
                headers.insert(name.as_str().to_ascii_lowercase(), v_str.to_string());
            }
        }

        let body = response
            .into_body()
            .read_to_vec()
            .map_err(|e| NetworkError::IoError(format!("Failed to read response body: {e}")))?;

        Ok(HttpResponse {
            final_url: url.clone(),
            status,
            status_text,
            headers,
            version: crate::http2::HttpVersion::Http11,
            body,
        })
    }

    /// Fetches a URL, following HTTP redirects up to `max_redirects` times.
    pub fn fetch(&self, start_url: &Url) -> Result<HttpResponse, NetworkError> {
        let mut current_url = start_url.clone();
        let mut redirects_followed = 0;
        let max_redirects = 5;
        let mut visited_urls = Vec::new();

        loop {
            if redirects_followed >= max_redirects {
                return Err(NetworkError::TooManyRedirects);
            }

            visited_urls.push(current_url.as_str());

            let resp = self.get_single(&current_url)?;

            if resp.is_redirect()
                && let Some(loc) = resp.location()
            {
                let next_url = current_url
                    .resolve(loc)
                    .map_err(|e| NetworkError::InvalidUrl(e.to_string()))?;

                if visited_urls.contains(&next_url.as_str()) {
                    return Err(NetworkError::TooManyRedirects); // Loop detected
                }

                log::info!("Following redirect {} -> {}", current_url, next_url);
                current_url = next_url;
                redirects_followed += 1;
                continue;
            }

            let mut final_resp = resp;
            final_resp.final_url = current_url;
            return Ok(final_resp);
        }
    }

    /// Sends an HTTP POST request with the given body bytes and Content-Type.
    ///
    /// Handles cookies and follows 301/302/303 Post/Redirect/Get redirects.
    pub fn post_with_body(
        &self,
        url: &Url,
        body: &[u8],
        content_type: &str,
    ) -> Result<HttpResponse, NetworkError> {
        let url_str = url.as_str();

        let mut req = self.agent.post(&url_str)
            .header("User-Agent", &self.user_agent)
            .header("Content-Type", content_type)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("sec-ch-ua", "\"Chromium\";v=\"128\", \"Not;A=Brand\";v=\"24\", \"Google Chrome\";v=\"128\"")
            .header("sec-ch-ua-mobile", "?0")
            .header("sec-ch-ua-platform", "\"Windows\"")
            .header("upgrade-insecure-requests", "1");

        if let Ok(jar) = self.cookie_jar.lock()
            && let Some(cookie_hdr) = jar.get_cookie_header(url)
        {
            req = req.header("Cookie", &cookie_hdr);
        }

        let resp = req.send(body).map_err(|e| match e {
            ureq::Error::Timeout(_) => NetworkError::Timeout,
            ureq::Error::HostNotFound => NetworkError::DnsResolutionFailed(
                format!("Host not found: {}", url.host.as_deref().unwrap_or("")),
            ),
            ureq::Error::ConnectionFailed => {
                NetworkError::ConnectionFailed("Connection failed".to_string())
            }
            ureq::Error::Tls(msg) => NetworkError::TlsError(msg.to_string()),
            ureq::Error::StatusCode(code) => {
                NetworkError::HttpError(code, format!("HTTP {code}"))
            }
            other => NetworkError::Other(other.to_string()),
        })?;

        // Extract Set-Cookie headers into the cookie jar
        if let Ok(mut jar) = self.cookie_jar.lock() {
            for (name, val) in resp.headers() {
                if name.as_str().eq_ignore_ascii_case("set-cookie")
                    && let Ok(v_str) = val.to_str()
                    && let Some(cookie) = CookieJar::parse_set_cookie(v_str, url)
                {
                    jar.store_cookie(cookie);
                }
            }
        }

        let status = resp.status().as_u16();
        let status_text = resp
            .status()
            .canonical_reason()
            .unwrap_or("")
            .to_string();

        let mut headers = HashMap::new();
        for (name, val) in resp.headers() {
            if let Ok(v_str) = val.to_str() {
                headers.insert(name.as_str().to_ascii_lowercase(), v_str.to_string());
            }
        }

        // Handle Post/Redirect/Get (301, 302, 303)
        if (status == 301 || status == 302 || status == 303)
            && let Some(loc) = headers.get("location")
        {
            if let Ok(redirect_url) = url.resolve(loc) {
                return self.fetch(&redirect_url);
            }
        }

        let body_bytes = resp
            .into_body()
            .read_to_vec()
            .map_err(|e| NetworkError::IoError(format!("Failed to read POST response body: {e}")))?;

        Ok(HttpResponse {
            final_url: url.clone(),
            status,
            status_text,
            headers,
            version: crate::http2::HttpVersion::Http11,
            body: body_bytes,
        })
    }

    /// Executes a general HTTP request with arbitrary method, custom headers, and optional body.
    ///
    /// Transmits cookies from the cookie jar, handles custom headers, records Set-Cookie responses,
    /// and returns status and body for all responses (including 4xx and 5xx).
    pub fn request(
        &self,
        method: &str,
        url: &Url,
        custom_headers: &[(String, String)],
        body: Option<&[u8]>,
    ) -> Result<HttpResponse, NetworkError> {
        let url_str = url.as_str();
        let upper_method = method.trim().to_uppercase();

        let resp = if upper_method == "GET" && body.is_none() {
            let mut req = self.agent
                .get(&url_str)
                .header("User-Agent", &self.user_agent)
                .header("Accept", "*/*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Accept-Encoding", "gzip, deflate, br");
            if let Ok(jar) = self.cookie_jar.lock() {
                if let Some(cookie_hdr) = jar.get_cookie_header_with_context(url, None, true, true) {
                    req = req.header("Cookie", &cookie_hdr);
                }
            }
            for (k, v) in custom_headers {
                req = req.header(k.as_str(), v.as_str());
            }
            req.call()
        } else if upper_method == "HEAD" {
            let mut req = self.agent
                .head(&url_str)
                .header("User-Agent", &self.user_agent)
                .header("Accept", "*/*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Accept-Encoding", "gzip, deflate, br");
            if let Ok(jar) = self.cookie_jar.lock() {
                if let Some(cookie_hdr) = jar.get_cookie_header_with_context(url, None, true, true) {
                    req = req.header("Cookie", &cookie_hdr);
                }
            }
            for (k, v) in custom_headers {
                req = req.header(k.as_str(), v.as_str());
            }
            req.call()
        } else if upper_method == "DELETE" {
            let mut req = self.agent
                .delete(&url_str)
                .header("User-Agent", &self.user_agent)
                .header("Accept", "*/*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Accept-Encoding", "gzip, deflate, br");
            if let Ok(jar) = self.cookie_jar.lock() {
                if let Some(cookie_hdr) = jar.get_cookie_header_with_context(url, None, true, true) {
                    req = req.header("Cookie", &cookie_hdr);
                }
            }
            for (k, v) in custom_headers {
                req = req.header(k.as_str(), v.as_str());
            }
            req.call()
        } else {
            let mut req = match upper_method.as_str() {
                "PUT" => self.agent.put(&url_str),
                "PATCH" => self.agent.patch(&url_str),
                _ => self.agent.post(&url_str),
            };
            req = req
                .header("User-Agent", &self.user_agent)
                .header("Accept", "*/*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Accept-Encoding", "gzip, deflate, br");
            if let Ok(jar) = self.cookie_jar.lock() {
                if let Some(cookie_hdr) = jar.get_cookie_header_with_context(url, None, true, true) {
                    req = req.header("Cookie", &cookie_hdr);
                }
            }
            for (k, v) in custom_headers {
                req = req.header(k.as_str(), v.as_str());
            }
            match body {
                Some(b) => req.send(b),
                None => req.send_empty(),
            }
        }.map_err(|e| match e {
            ureq::Error::Timeout(_) => NetworkError::Timeout,
            ureq::Error::HostNotFound => NetworkError::DnsResolutionFailed(
                format!("Host not found: {}", url.host.as_deref().unwrap_or("")),
            ),
            ureq::Error::ConnectionFailed => {
                NetworkError::ConnectionFailed("Connection failed".to_string())
            }
            ureq::Error::Tls(msg) => NetworkError::TlsError(msg.to_string()),
            ureq::Error::StatusCode(code) => {
                NetworkError::HttpError(code, format!("HTTP {code}"))
            }
            other => NetworkError::Other(other.to_string()),
        })?;

        // Extract Set-Cookie headers into the cookie jar
        if let Ok(mut jar) = self.cookie_jar.lock() {
            for (name, val) in resp.headers() {
                if name.as_str().eq_ignore_ascii_case("set-cookie")
                    && let Ok(v_str) = val.to_str()
                    && let Some(cookie) = CookieJar::parse_set_cookie(v_str, url)
                {
                    jar.store_cookie(cookie);
                }
            }
        }

        let status = resp.status().as_u16();
        let status_text = resp
            .status()
            .canonical_reason()
            .unwrap_or("")
            .to_string();

        let mut headers = HashMap::new();
        for (name, val) in resp.headers() {
            if let Ok(v_str) = val.to_str() {
                headers.insert(name.as_str().to_ascii_lowercase(), v_str.to_string());
            }
        }

        let body_bytes = resp
            .into_body()
            .read_to_vec()
            .map_err(|e| NetworkError::IoError(format!("Failed to read response body: {e}")))?;

        Ok(HttpResponse {
            final_url: url.clone(),
            status,
            status_text,
            headers,
            version: crate::http2::HttpVersion::Http11,
            body: body_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_response_helpers() {
        let mut headers = HashMap::new();
        headers.insert(
            "content-type".to_string(),
            "text/html; charset=UTF-8".to_string(),
        );
        headers.insert("location".to_string(), "https://example.com/dest".to_string());

        let resp = HttpResponse {
            final_url: Url::parse("https://example.com").unwrap(),
            status: 301,
            status_text: "Moved Permanently".to_string(),
            headers,
            version: crate::http2::HttpVersion::Http11,
            body: b"Redirecting...".to_vec(),
        };

        assert!(resp.is_redirect());
        assert!(!resp.is_success());
        assert_eq!(resp.content_type(), Some("text/html"));
        assert_eq!(resp.location(), Some("https://example.com/dest"));
        assert_eq!(resp.body_as_string(), "Redirecting...");
    }

    #[test]
    fn test_cookie_jar_storage_and_header_generation() {
        let mut jar = CookieJar::new();
        let url = Url::parse("https://duckduckgo.com/settings").unwrap();

        let cookie1 = CookieJar::parse_set_cookie(
            "p=1; Path=/; Domain=duckduckgo.com; Secure",
            &url,
        ).unwrap();
        let cookie2 = CookieJar::parse_set_cookie(
            "theme=dark; Path=/settings; HttpOnly",
            &url,
        ).unwrap();

        jar.store_cookie(cookie1);
        jar.store_cookie(cookie2);

        let hdr = jar.get_cookie_header(&url).unwrap();
        assert!(hdr.contains("p=1"));
        assert!(hdr.contains("theme=dark"));

        // Path mismatch for /other
        let other_url = Url::parse("https://duckduckgo.com/other").unwrap();
        let other_hdr = jar.get_cookie_header(&other_url).unwrap();
        assert_eq!(other_hdr, "p=1");

        // Subdomain matching
        let sub_url = Url::parse("https://lite.duckduckgo.com/").unwrap();
        let sub_hdr = jar.get_cookie_header(&sub_url).unwrap();
        assert_eq!(sub_hdr, "p=1");

        // Deletion via Max-Age=0
        let delete_cookie = CookieJar::parse_set_cookie(
            "p=deleted; Max-Age=0; Domain=duckduckgo.com; Path=/",
            &url,
        ).unwrap();
        jar.store_cookie(delete_cookie);

        let after_delete = jar.get_cookie_header(&url).unwrap();
        assert_eq!(after_delete, "theme=dark");
    }
}
