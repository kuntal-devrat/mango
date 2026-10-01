//! Navigation subsystem for Mango Browser (OPT-001).
//!
//! Provides URL parsing, search query detection, percent-encoding,
//! special page routing, and connection error page generation.

/// HTML-escapes text for safe interpolation into error and status pages.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Percent-encodes a string for URL query parameters (RFC 1866 application/x-www-form-urlencoded).
pub fn url_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() * 2);
    for ch in input.chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                encoded.push(ch);
            }
            ' ' => {
                encoded.push('+');
            }
            _ => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                let mut buf = [0u8; 4];
                let utf8_bytes = ch.encode_utf8(&mut buf);
                for b in utf8_bytes.bytes() {
                    encoded.push('%');
                    encoded.push(HEX[(b >> 4) as usize] as char);
                    encoded.push(HEX[(b & 0x0F) as usize] as char);
                }
            }
        }
    }
    encoded
}

/// Determines if the given address bar input represents a search query
/// rather than a direct URL or local resource.
pub fn is_search_query(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();

    // Explicit schemes (case-insensitive)
    if let Some((scheme, rest)) = lower.split_once(':')
        && !scheme.contains('/')
        && !scheme.contains(' ')
    {
        match scheme {
            "http" | "https" | "ftp" | "file" | "about" | "test" | "data" | "view-source"
            | "javascript" | "mailto" | "blob" | "tel" => {
                return false;
            }
            _ => {
                if rest.starts_with("//") {
                    return false;
                }
            }
        }
    }

    // Contains whitespace -> search query
    if trimmed.contains(char::is_whitespace) {
        return true;
    }

    // Localhost or loopback (e.g. localhost:3000, 127.0.0.1:8080)
    if lower.starts_with("localhost") || lower.starts_with("127.0.0.1") {
        return false;
    }

    // IPv6 bracketed or raw
    if trimmed.starts_with('[') {
        return false;
    }
    if lower == "::1"
        || trimmed
            .split('/')
            .next()
            .unwrap_or("")
            .parse::<std::net::Ipv6Addr>()
            .is_ok()
    {
        return false;
    }

    // Host part before path or query
    let host_part = trimmed.split(['/', '?', '#']).next().unwrap_or(trimmed);

    // IPv4 check: e.g. 192.168.1.1 or 192.168.1.1:8080
    let ip_str = if let Some((h, _)) = host_part.split_once(':') {
        h
    } else {
        host_part
    };
    if ip_str.parse::<std::net::Ipv4Addr>().is_ok() {
        return false;
    }

    // Host with explicit port e.g. my-machine:3000
    if let Some((h, port_str)) = host_part.split_once(':')
        && !h.is_empty()
        && port_str.parse::<u16>().is_ok()
    {
        return false;
    }

    // Domain check: must contain dot, and not start/end with dot
    if !host_part.contains('.') || host_part.starts_with('.') || host_part.ends_with('.') {
        return true;
    }

    false
}

/// Tests whether the input is a special protocol, raw HTML, or local test suite.
pub fn is_special_or_local_page(input: &str) -> bool {
    let lower = input.trim().to_ascii_lowercase();
    input.starts_with('<')
        || lower.starts_with("data:")
        || lower.starts_with("about:")
        || lower.starts_with("test:")
}

/// Target destination resolved from raw omnibox input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationTarget {
    SpecialOrLocal(String),
    SearchQuery(String),
    NetworkUrl(String),
}

/// Resolves raw omnibox input to its intended navigation destination with default configuration.
pub fn resolve_omnibox_input(input: &str) -> NavigationTarget {
    resolve_omnibox_input_with_config(input, &crate::config::Config::default())
}

/// Resolves raw omnibox input using the provided browser configuration.
pub fn resolve_omnibox_input_with_config(
    input: &str,
    config: &crate::config::Config,
) -> NavigationTarget {
    let trimmed = input.trim();
    if is_special_or_local_page(trimmed) {
        NavigationTarget::SpecialOrLocal(trimmed.to_string())
    } else if is_search_query(trimmed) {
        NavigationTarget::SearchQuery(config.format_search_url(trimmed))
    } else {
        NavigationTarget::NetworkUrl(trimmed.to_string())
    }
}

/// Generates a styled, responsive connection error page with safe HTML escaping.
pub fn error_page_html(target_url: &str, error_message: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Connection Error - Mango</title>
    <style>
        body {{ background-color: #f8fafc; margin: 24px; color: #1e293b; }}
        .error-box {{
            background-color: #ffffff;
            border-left-width: 4px;
            border-left-color: #ef4444;
            border-top-width: 1px;
            border-right-width: 1px;
            border-bottom-width: 1px;
            border-top-color: #e2e8f0;
            border-right-color: #e2e8f0;
            border-bottom-color: #e2e8f0;
            padding: 20px;
            margin-bottom: 16px;
        }}
        h1 {{ font-size: 22px; color: #dc2626; margin: 0px 0px 8px 0px; }}
        .url {{ font-size: 13px; color: #64748b; margin: 0px 0px 14px 0px; }}
        .message {{
            background-color: #fef2f2;
            color: #991b1b;
            padding: 10px;
            font-size: 13px;
            margin: 8px 0px;
        }}
        .help {{
            background-color: #ffffff;
            border-top-width: 1px;
            border-right-width: 1px;
            border-bottom-width: 1px;
            border-left-width: 1px;
            border-top-color: #e2e8f0;
            border-right-color: #e2e8f0;
            border-bottom-color: #e2e8f0;
            border-left-color: #e2e8f0;
            padding: 16px;
        }}
        h2 {{ font-size: 16px; color: #334155; margin: 0px 0px 8px 0px; }}
        p {{ font-size: 13px; color: #475569; margin: 4px 0px; }}
    </style>
</head>
<body>
    <div class="error-box">
        <h1>Could Not Connect</h1>
        <p class="url">Target: {}</p>
        <div class="message">{}</div>
    </div>
    <div class="help">
        <h2>Troubleshooting Suggestions</h2>
        <p>- Verify that your internet connection is active.</p>
        <p>- Check the URL spelling (e.g. https://example.com).</p>
        <p>- If the website uses an untrusted or expired TLS certificate, Mango's pure-Rust TLS validator safely refuses connection.</p>
        <p>- Return to Mango home by entering "about:welcome" in the address bar.</p>
    </div>
</body>
</html>"#,
        html_escape(target_url),
        html_escape(error_message)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encoding() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("rust & web"), "rust+%26+web");
        assert_eq!(url_encode("🦀"), "%F0%9F%A6%80");
    }

    #[test]
    fn test_search_query_detection() {
        assert!(is_search_query("what is rust"));
        assert!(is_search_query("mango"));
        assert!(!is_search_query("https://google.com"));
        assert!(!is_search_query("HTTP://google.com"));
        assert!(!is_search_query("http://localhost:8080"));
        assert!(!is_search_query("localhost:3000"));
        assert!(!is_search_query("example.com"));
        assert!(!is_search_query("192.168.1.1"));
        assert!(!is_search_query("192.168.1.1:8080"));
        assert!(!is_search_query("[::1]"));
        assert!(!is_search_query("[::1]:8080"));
        assert!(!is_search_query("::1"));
        assert!(!is_search_query("about:blank"));
        assert!(!is_search_query("test:forms"));
    }

    #[test]
    fn test_error_page_html_escaping() {
        let html = error_page_html("<script>alert(1)</script>", "error & fail");
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains("error &amp; fail"));
    }
}
