//! URL parsing and relative URL resolution backed by the WHATWG `url` standard crate.

use std::fmt;
use std::str::FromStr;

/// Errors that can occur during URL parsing or resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlError {
    EmptyUrl,
    InvalidScheme,
    MissingHost,
    InvalidPort(String),
    RelativeWithoutBase,
    Malformed(String),
}

impl fmt::Display for UrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UrlError::EmptyUrl => write!(f, "URL is empty"),
            UrlError::InvalidScheme => write!(f, "URL has an invalid or unsupported scheme"),
            UrlError::MissingHost => write!(f, "URL is missing a valid host"),
            UrlError::InvalidPort(p) => write!(f, "Invalid port number: '{p}'"),
            UrlError::RelativeWithoutBase => {
                write!(f, "Cannot resolve a relative URL without a base URL")
            }
            UrlError::Malformed(msg) => write!(f, "Malformed URL: {msg}"),
        }
    }
}

impl std::error::Error for UrlError {}

/// A parsed Uniform Resource Locator (URL).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Url {
    /// Scheme in lowercase (e.g. "http", "https", "about", "data", "test", "file").
    pub scheme: String,
    /// Host in lowercase (e.g. "example.com", "127.0.0.1"), if applicable.
    pub host: Option<String>,
    /// Explicit non-default port number. Default ports (80 for http/ws,
    /// 443 for https/wss) are normalized to `None` for equality.
    pub port: Option<u16>,
    /// Path component (always starts with '/' for network URLs).
    pub path: String,
    /// Query string without the leading '?', if present.
    pub query: Option<String>,
    /// Fragment identifier without the leading '#', if present.
    pub fragment: Option<String>,
    /// Username from userinfo (`scheme://user:pass@host`), if present.
    pub username: Option<String>,
    /// Password from userinfo, if present.
    pub password: Option<String>,
}

/// Returns true if `scheme` matches ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )
/// per RFC 3986 §3.1.
pub fn is_valid_scheme(scheme: &str) -> bool {
    let mut chars = scheme.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
}

impl Url {
    /// Parses a URL string.
    ///
    /// If the user inputs a bare domain like `example.com` or `localhost:8080`,
    /// this function automatically normalizes it to `https://` (or `http://` for localhost).
    pub fn parse(input: &str) -> Result<Self, UrlError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(UrlError::EmptyUrl);
        }

        // Custom browser schemes (case-insensitive per RFC 3986 §3.1)
        let lower_prefix = trimmed.to_ascii_lowercase();
        for scheme_name in [
            "about:",
            "test:",
            "data:",
            "javascript:",
            "mailto:",
            "blob:",
            "tel:",
        ] {
            if lower_prefix.starts_with(scheme_name) {
                let rest = trimmed[scheme_name.len()..].to_string();
                let scheme = scheme_name.trim_end_matches(':').to_string();
                return Ok(Url {
                    scheme,
                    host: None,
                    port: None,
                    path: rest,
                    query: None,
                    fragment: None,
                    username: None,
                    password: None,
                });
            }
        }

        // Auto-normalize bare domains or protocol-less inputs
        let normalized = if !trimmed.contains("://") {
            if trimmed.starts_with("//") {
                format!("https:{trimmed}")
            } else if trimmed.starts_with("localhost")
                || trimmed.starts_with("127.0.0.1")
                || trimmed.starts_with('[')
                || trimmed.starts_with("::1")
            {
                format!("http://{trimmed}")
            } else {
                format!("https://{trimmed}")
            }
        } else {
            trimmed.to_string()
        };

        // Extract and validate scheme
        let scheme_end = normalized.find("://").ok_or(UrlError::InvalidScheme)?;
        let raw_scheme = &normalized[..scheme_end];
        if !is_valid_scheme(raw_scheme) {
            return Err(UrlError::InvalidScheme);
        }

        // Check for empty userinfo (e.g. `http://@example.com`)
        let after_scheme = &normalized[scheme_end + 3..];
        let authority = match after_scheme.find(['/', '?', '#']) {
            Some(idx) => &after_scheme[..idx],
            None => after_scheme,
        };
        if let Some(userinfo) = authority.rfind('@').map(|i| &authority[..i]) {
            if userinfo.is_empty() {
                return Err(UrlError::Malformed(
                    "Empty userinfo in authority".to_string(),
                ));
            }
        }

        let parsed = url::Url::parse(&normalized).map_err(|e| match e {
            url::ParseError::EmptyHost => UrlError::MissingHost,
            url::ParseError::InvalidPort => UrlError::InvalidPort("Invalid port".to_string()),
            _ => UrlError::Malformed(e.to_string()),
        })?;

        let scheme = parsed.scheme().to_ascii_lowercase();
        let host = parsed.host_str().map(|h| h.to_string());

        // url crate normalizes default ports (80 for http/ws, 443 for https/wss) to None!
        let port = parsed.port();
        let path = parsed.path().to_string();
        let query = parsed.query().map(|q| q.to_string());
        let fragment = parsed.fragment().map(|f| f.to_string());
        let username = if parsed.username().is_empty() {
            None
        } else {
            Some(parsed.username().to_string())
        };
        let password = parsed.password().map(|p| p.to_string());

        Ok(Url {
            scheme,
            host,
            port,
            path,
            query,
            fragment,
            username,
            password,
        })
    }

    /// Resolves a relative URL reference against this base URL according to WHATWG URL Standard.
    pub fn resolve(&self, relative: &str) -> Result<Self, UrlError> {
        let rel = relative.trim();
        if rel.is_empty() {
            return Ok(self.clone());
        }

        // If relative has an explicit custom or network scheme, parse directly
        let lower = rel.to_ascii_lowercase();
        for scheme_name in [
            "about:",
            "test:",
            "data:",
            "javascript:",
            "mailto:",
            "blob:",
            "tel:",
            "http:",
            "https:",
        ] {
            if lower.starts_with(scheme_name) {
                return Self::parse(rel);
            }
        }

        if rel.contains("://") {
            return Self::parse(rel);
        }

        let base_str = self.as_str();
        let base_url = url::Url::parse(&base_str).map_err(|_| UrlError::RelativeWithoutBase)?;
        let resolved = base_url
            .join(rel)
            .map_err(|e| UrlError::Malformed(e.to_string()))?;
        Self::parse(resolved.as_str())
    }

    /// Returns the origin string formatted as `scheme://host[:port]`.
    /// Userinfo is excluded per RFC 6454; default ports are omitted.
    pub fn origin(&self) -> String {
        match (&self.host, self.port) {
            (Some(host), Some(port)) => {
                format!("{}://{}:{}", self.scheme, host, port)
            }
            (Some(host), None) => format!("{}://{}", self.scheme, host),
            (None, _) => format!("{}:", self.scheme),
        }
    }

    /// Reconstitutes the canonical string representation of the URL.
    /// Includes userinfo when present.
    pub fn as_str(&self) -> String {
        let mut out = String::new();
        if let Some(host) = self.host.as_deref() {
            out.push_str(&self.scheme);
            out.push_str("://");
            if let Some(user) = self.username.as_deref() {
                out.push_str(user);
                if let Some(pass) = self.password.as_deref() {
                    out.push(':');
                    out.push_str(pass);
                }
                out.push('@');
            }
            out.push_str(host);
            if let Some(port) = self.port {
                out.push(':');
                out.push_str(&port.to_string());
            }
            out.push_str(&self.path);
        } else {
            out.push_str(&self.scheme);
            out.push(':');
            out.push_str(&self.path);
        }

        if let Some(q) = &self.query {
            out.push('?');
            out.push_str(q);
        }
        if let Some(f) = &self.fragment {
            out.push('#');
            out.push_str(f);
        }
        out
    }

    /// Returns the path and query string (e.g. `/index.html?q=1`) for HTTP request lines.
    pub fn path_and_query(&self) -> String {
        match &self.query {
            Some(q) => format!("{}?{}", self.path, q),
            None => self.path.clone(),
        }
    }

    /// Returns `true` if this is an HTTP or HTTPS URL.
    pub fn is_http_or_https(&self) -> bool {
        self.scheme == "http" || self.scheme == "https"
    }

    /// Returns the effective port (e.g. 80 for http, 443 for https).
    pub fn effective_port(&self) -> u16 {
        self.port.unwrap_or(match self.scheme.as_str() {
            "http" | "ws" => 80,
            "https" | "wss" => 443,
            "ftp" => 21,
            _ => 80,
        })
    }

    /// Returns the origin URL string with trailing slash (e.g. `https://example.com/` or `https://example.com:8080/`).
    pub fn origin_url(&self) -> Option<String> {
        let host = self.host.as_deref()?;
        let mut out = format!("{}://{}", self.scheme, host);
        if let Some(port) = self.port {
            out.push(':');
            out.push_str(&port.to_string());
        }
        out.push('/');
        Some(out)
    }

    /// Serializes this URL stripped for HTTP Referer: without userinfo (username/password) and without fragment.
    pub fn stripped_for_referrer(&self) -> Option<String> {
        let host = self.host.as_deref()?;
        let mut out = format!("{}://{}", self.scheme, host);
        if let Some(port) = self.port {
            out.push(':');
            out.push_str(&port.to_string());
        }
        if self.path.is_empty() {
            out.push('/');
        } else {
            out.push_str(&self.path);
        }
        if let Some(q) = &self.query {
            out.push('?');
            out.push_str(q);
        }
        Some(out)
    }
}

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Url {
    type Err = UrlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_https_url() {
        let url = Url::parse("https://example.com/blog/article?id=123#comments").unwrap();
        assert_eq!(url.scheme, "https");
        assert_eq!(url.host.as_deref(), Some("example.com"));
        // Default ports normalize to None for equality.
        assert_eq!(url.port, None);
        assert_eq!(url.effective_port(), 443);
        assert_eq!(url.path, "/blog/article");
        assert_eq!(url.query.as_deref(), Some("id=123"));
        assert_eq!(url.fragment.as_deref(), Some("comments"));
        assert_eq!(
            url.as_str(),
            "https://example.com/blog/article?id=123#comments"
        );
        // Explicit default port equals implicit.
        let explicit = Url::parse("https://example.com:443/blog/article?id=123#comments").unwrap();
        assert_eq!(url, explicit);
    }

    #[test]
    fn test_auto_normalize_bare_domain() {
        let url = Url::parse("example.com").unwrap();
        assert_eq!(url.scheme, "https");
        assert_eq!(url.host.as_deref(), Some("example.com"));
        assert_eq!(url.path, "/");
        assert_eq!(url.as_str(), "https://example.com/");
    }

    #[test]
    fn test_parse_explicit_port() {
        let url = Url::parse("http://localhost:8080/api").unwrap();
        assert_eq!(url.scheme, "http");
        assert_eq!(url.host.as_deref(), Some("localhost"));
        assert_eq!(url.port, Some(8080));
        assert_eq!(url.path, "/api");
        assert_eq!(url.as_str(), "http://localhost:8080/api");
    }

    #[test]
    fn test_custom_schemes() {
        let about = Url::parse("about:welcome").unwrap();
        assert_eq!(about.scheme, "about");
        assert_eq!(about.path, "welcome");

        let test_box = Url::parse("test:box").unwrap();
        assert_eq!(test_box.scheme, "test");
        assert_eq!(test_box.path, "box");
    }

    #[test]
    fn test_resolve_relative_paths() {
        let base = Url::parse("https://example.com/blog/2026/post.html").unwrap();

        // Sibling file
        let rel1 = base.resolve("style.css").unwrap();
        assert_eq!(rel1.as_str(), "https://example.com/blog/2026/style.css");

        // Root-relative path
        let rel2 = base.resolve("/assets/logo.png").unwrap();
        assert_eq!(rel2.as_str(), "https://example.com/assets/logo.png");

        // Parent directory
        let rel3 = base.resolve("../common.css").unwrap();
        assert_eq!(rel3.as_str(), "https://example.com/blog/common.css");

        // Protocol-relative
        let rel4 = base.resolve("//cdn.example.org/font.woff2").unwrap();
        assert_eq!(rel4.as_str(), "https://cdn.example.org/font.woff2");

        // Query only
        let rel5 = base.resolve("?page=2").unwrap();
        assert_eq!(
            rel5.as_str(),
            "https://example.com/blog/2026/post.html?page=2"
        );

        // Absolute schemes resolved against base URL
        let rel_js = base.resolve("javascript:alert(1)").unwrap();
        assert_eq!(rel_js.scheme, "javascript");
        assert_eq!(rel_js.path, "alert(1)");

        let rel_mailto = base.resolve("mailto:user@example.com").unwrap();
        assert_eq!(rel_mailto.scheme, "mailto");
        assert_eq!(rel_mailto.path, "user@example.com");

        let rel_blob = base.resolve("blob:https://example.com/uuid").unwrap();
        assert_eq!(rel_blob.scheme, "blob");
    }

    #[test]
    fn test_ipv6_url_parsing() {
        let url1 = Url::parse("http://[::1]/index.html").unwrap();
        assert_eq!(url1.scheme, "http");
        assert_eq!(url1.host.as_deref(), Some("[::1]"));
        assert_eq!(url1.port, None);
        assert_eq!(url1.effective_port(), 80);
        assert_eq!(url1.path, "/index.html");

        let url2 = Url::parse("http://[::1]:8080/api").unwrap();
        assert_eq!(url2.scheme, "http");
        assert_eq!(url2.host.as_deref(), Some("[::1]"));
        assert_eq!(url2.port, Some(8080));
        assert_eq!(url2.path, "/api");

        let url3 = Url::parse("[::1]:3000").unwrap();
        assert_eq!(url3.scheme, "http");
        assert_eq!(url3.host.as_deref(), Some("[::1]"));
        assert_eq!(url3.port, Some(3000));
    }

    #[test]
    fn test_scheme_case_insensitive_and_validation() {
        let upper = Url::parse("HTTP://Example.COM/Path").unwrap();
        assert_eq!(upper.scheme, "http");
        assert_eq!(upper.host.as_deref(), Some("example.com"));
        let data_upper = Url::parse("DATA:text/plain,hi").unwrap();
        assert_eq!(data_upper.scheme, "data");
        assert!(Url::parse("1http://example.com/").is_err());
        assert!(Url::parse("ht tp://example.com/").is_err());
        assert!(is_valid_scheme("https"));
        assert!(is_valid_scheme("a+b-c.d"));
        assert!(!is_valid_scheme("1abc"));
        assert!(!is_valid_scheme(""));
    }

    #[test]
    fn test_userinfo_preserved() {
        let u = Url::parse("https://user:pass@example.com/path").unwrap();
        assert_eq!(u.username.as_deref(), Some("user"));
        assert_eq!(u.password.as_deref(), Some("pass"));
        assert_eq!(u.host.as_deref(), Some("example.com"));
        assert!(u.as_str().contains("user:pass@"));
        // Origin must exclude credentials.
        assert_eq!(u.origin(), "https://example.com");
        let u2 = Url::parse("https://bob@example.com/").unwrap();
        assert_eq!(u2.username.as_deref(), Some("bob"));
        assert_eq!(u2.password, None);
    }

    #[test]
    fn test_default_port_normalization() {
        let a = Url::parse("https://example.com/").unwrap();
        let b = Url::parse("https://example.com:443/").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.port, None);
        let c = Url::parse("http://example.com/").unwrap();
        let d = Url::parse("http://example.com:80/").unwrap();
        assert_eq!(c, d);
        let e = Url::parse("https://example.com:8443/").unwrap();
        assert_ne!(a, e);
    }
}
