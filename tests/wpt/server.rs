//! Static HTTP server for running canonical Web Platform Tests (WPT) and Acid3 locally.
//!
//! Provides a local HTTP/1.1 server serving the WPT test suite and Acid3 fixtures with:
//! - CORS and timing headers.
//! - Companion `.headers` file reading and response header injection.
//! - WPT query string pipe handling (`?pipe=status(NNN)`, `?pipe=header(K,V)`).
//! - Automatic routing for `/acid/acid3/` and `/acid3/` paths.
//! - Proper MIME types and resources mapping (`/resources/testharness.js`, etc.).

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub struct WptServer {
    port: u16,
    base_dir: PathBuf,
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl WptServer {
    /// Starts a new local static HTTP server serving files rooted at `base_dir`.
    pub fn new(base_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let port = addr.port();
        let base_dir = base_dir.as_ref().to_path_buf();
        let running = Arc::new(AtomicBool::new(true));

        let s_running = running.clone();
        let s_base = base_dir.clone();
        let handle = thread::Builder::new()
            .name("wpt-http-server".to_string())
            .spawn(move || {
                while s_running.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let base_dir = s_base.clone();
                            thread::spawn(move || {
                                let _ = handle_client(stream, &base_dir);
                            });
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            })?;

        Ok(Self {
            port,
            base_dir,
            running,
            handle: Some(handle),
        })
    }

    /// The base URL of the running WPT server (e.g. `http://127.0.0.1:41234`).
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Constructs a fully qualified URL for a relative path.
    pub fn url_for(&self, subpath: &str) -> String {
        let clean = subpath.trim_start_matches('/');
        format!("http://127.0.0.1:{}/{}", self.port, clean)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

impl Drop for WptServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn handle_client(mut stream: TcpStream, base_dir: &Path) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return Ok(()),
    };

    let req_str = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req_str.lines();
    let Some(first_line) = lines.next() else {
        return Ok(());
    };

    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let full_path = parts.next().unwrap_or("/");

    if method == "OPTIONS" {
        let resp = "HTTP/1.1 204 No Content\r\n\
                    Access-Control-Allow-Origin: *\r\n\
                    Access-Control-Allow-Methods: GET, POST, OPTIONS, PUT, DELETE, HEAD\r\n\
                    Access-Control-Allow-Headers: *\r\n\
                    Access-Control-Max-Age: 86400\r\n\
                    Connection: close\r\n\r\n";
        let _ = stream.write_all(resp.as_bytes());
        return Ok(());
    }

    // Separate fragment (#) and query string (?)
    let (path_and_query, _) = full_path.split_once('#').unwrap_or((full_path, ""));
    let (path_part, query_part) = path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""));
    let clean_path = path_part.trim_start_matches('/');

    // Parse WPT pipes from query string
    let (pipe_status, pipe_headers) = parse_query_pipes(query_part);

    // Resolve target file
    let target_file_opt = resolve_target_file(base_dir, clean_path);

    if let Some(target_file) = target_file_opt
        && let Ok(bytes) = std::fs::read(&target_file)
    {
        let status_code = pipe_status.unwrap_or(200);
        let reason = status_reason_phrase(status_code);

        // Build response headers
        let mut headers: Vec<(String, String)> = vec![
            (
                "Content-Type".to_string(),
                guess_mime_type(&target_file).to_string(),
            ),
            ("Access-Control-Allow-Origin".to_string(), "*".to_string()),
            (
                "Access-Control-Allow-Methods".to_string(),
                "GET, POST, OPTIONS".to_string(),
            ),
            ("Access-Control-Allow-Headers".to_string(), "*".to_string()),
            ("Timing-Allow-Origin".to_string(), "*".to_string()),
        ];

        // Companion .headers override/injection
        if let Some(headers_path) = companion_headers_path(&target_file) {
            for (k, v) in parse_headers_file(&headers_path) {
                set_header(&mut headers, &k, &v);
            }
        }

        // Query pipe header overrides
        for (k, v) in pipe_headers {
            set_header(&mut headers, &k, &v);
        }

        set_header(&mut headers, "Content-Length", &bytes.len().to_string());
        set_header(&mut headers, "Connection", "close");

        let mut resp = format!("HTTP/1.1 {} {}\r\n", status_code, reason);
        for (k, v) in &headers {
            resp.push_str(&format!("{}: {}\r\n", k, v));
        }
        resp.push_str("\r\n");

        stream.write_all(resp.as_bytes())?;
        if method != "HEAD" && status_code != 204 && status_code != 304 {
            stream.write_all(&bytes)?;
        }
        return Ok(());
    }

    // 404 Not Found fallback
    let not_found_body = format!("404 Not Found: {}", path_part);
    let resp = format!(
        "HTTP/1.1 404 Not Found\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\r\n\
         {}",
        not_found_body.len(),
        not_found_body
    );
    let _ = stream.write_all(resp.as_bytes());
    Ok(())
}

/// Resolves a requested URL path to an existing local file on disk.
fn resolve_target_file(base_dir: &Path, clean_path: &str) -> Option<PathBuf> {
    // Sanitize against path traversal attacks: reject any path containing `..`,
    // root, or prefix components.
    let normalized = clean_path.replace('\\', "/");
    if Path::new(&normalized).components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }

    // 1. Special WPT reporter hook alias
    if clean_path == "resources/testharnessreport.js" {
        for candidate in [
            base_dir.join("testharnessreport.js"),
            base_dir.join("tests/wpt/testharnessreport.js"),
        ] {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // 2. Acid3 route aliases: /acid/acid3/* or /acid3/*
    let acid_subpath = if clean_path == "acid/acid3"
        || clean_path == "acid/acid3/"
        || clean_path == "acid3"
        || clean_path == "acid3/"
    {
        Some("test.html")
    } else {
        clean_path
            .strip_prefix("acid/acid3/")
            .or_else(|| clean_path.strip_prefix("acid3/"))
    };

    if let Some(sub) = acid_subpath {
        for candidate in [
            base_dir.join("acid3").join(sub),
            base_dir.join("tests/wpt/acid3").join(sub),
            base_dir.join(sub),
            base_dir.join("upstream/acid/acid3").join(sub),
        ] {
            if candidate.is_file() {
                return Some(candidate);
            }
            if candidate.is_dir() {
                let index = candidate.join("test.html");
                if index.is_file() {
                    return Some(index);
                }
            }
        }
    }

    // 3. Canonical upstream WPT tree
    let upstream_candidate = base_dir.join("upstream").join(clean_path);
    if upstream_candidate.is_file() {
        return Some(upstream_candidate);
    }
    let upstream_sub = base_dir.join("tests/wpt/upstream").join(clean_path);
    if upstream_sub.is_file() {
        return Some(upstream_sub);
    }

    // 4. Acid3 directory search (for relative asset references)
    let acid_direct = base_dir.join("acid3").join(clean_path);
    if acid_direct.is_file() {
        return Some(acid_direct);
    }
    let acid_sub_direct = base_dir.join("tests/wpt/acid3").join(clean_path);
    if acid_sub_direct.is_file() {
        return Some(acid_sub_direct);
    }

    // 5. Direct relative to base_dir
    let direct = base_dir.join(clean_path);
    if direct.is_file() {
        return Some(direct);
    }

    // 6. Direct relative to tests/wpt (if base_dir is workspace root)
    let wpt_direct = base_dir.join("tests/wpt").join(clean_path);
    if wpt_direct.is_file() {
        return Some(wpt_direct);
    }

    None
}

/// Locates companion `.headers` file for a given target file (e.g. `empty.css.headers`).
fn companion_headers_path(target_file: &Path) -> Option<PathBuf> {
    let file_name = target_file.file_name()?.to_str()?;
    let headers_name = format!("{}.headers", file_name);
    let candidate = target_file.with_file_name(headers_name);
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

/// Parses standard HTTP header lines (`Header-Name: Value`) from a companion `.headers` file.
fn parse_headers_file(path: &Path) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    let Ok(content) = std::fs::read_to_string(path) else {
        return headers;
    };
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            let key = name.trim().to_string();
            let val = value.trim().to_string();
            if !key.is_empty() {
                headers.push((key, val));
            }
        }
    }
    headers
}

/// Parses WPT pipe commands from query string (e.g. `pipe=status(404)` or `pipe=header(K,V)`).
fn parse_query_pipes(query_str: &str) -> (Option<u16>, Vec<(String, String)>) {
    let mut status = None;
    let mut custom_headers = Vec::new();

    if query_str.is_empty() {
        return (status, custom_headers);
    }

    for param in query_str.split('&') {
        if let Some((k, v)) = param.split_once('=')
            && k == "pipe"
        {
            for pipe_cmd in v.split('|') {
                let cmd = pipe_cmd.trim();
                if let Some(rest) = cmd.strip_prefix("status(") {
                    if let Some(code_str) = rest.strip_suffix(')')
                        && let Ok(code) = code_str.trim().parse::<u16>()
                    {
                        status = Some(code);
                    }
                } else if let Some(rest) = cmd.strip_prefix("header(")
                    && let Some(args) = rest.strip_suffix(')')
                    && let Some((h_name, h_val)) = args.split_once(',')
                {
                    let name = h_name.trim().to_string();
                    let val = h_val.trim().to_string();
                    if !name.is_empty() {
                        custom_headers.push((name, val));
                    }
                }
            }
        }
    }

    (status, custom_headers)
}

/// Sets or updates a header in case-insensitive manner.
fn set_header(headers: &mut Vec<(String, String)>, name: &str, value: &str) {
    if let Some(existing) = headers
        .iter_mut()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
    {
        existing.1 = value.to_string();
    } else {
        headers.push((name.to_string(), value.to_string()));
    }
}

/// Returns the HTTP/1.1 reason phrase for a given status code.
fn status_reason_phrase(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        206 => "Partial Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        415 => "Unsupported Media Type",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Custom Status",
    }
}

fn guess_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "xhtml" | "xht" => "application/xhtml+xml; charset=utf-8",
        "ico" => "image/x-icon",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_pipes_status_404() {
        let (status, headers) = parse_query_pipes("pipe=status(404)");
        assert_eq!(status, Some(404));
        assert!(headers.is_empty());
    }

    #[test]
    fn test_parse_query_pipes_combined_and_chained() {
        let (status, headers) =
            parse_query_pipes("v=1&pipe=header(Content-Type,text/plain)|status(404)&other=abc");
        assert_eq!(status, Some(404));
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "Content-Type");
        assert_eq!(headers[0].1, "text/plain");
    }

    #[test]
    fn test_companion_headers_parsing() {
        let temp_dir = std::env::temp_dir().join("mango_wpt_server_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let headers_file = temp_dir.join("empty.css.headers");
        std::fs::write(
            &headers_file,
            "Content-Type: text/html\r\nCache-Control: no-cache\r\n",
        )
        .unwrap();

        let parsed = parse_headers_file(&headers_file);
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed[0],
            ("Content-Type".to_string(), "text/html".to_string())
        );
        assert_eq!(
            parsed[1],
            ("Cache-Control".to_string(), "no-cache".to_string())
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_set_header_case_insensitive_override() {
        let mut headers = vec![
            ("Content-Type".to_string(), "text/css".to_string()),
            ("Access-Control-Allow-Origin".to_string(), "*".to_string()),
        ];
        set_header(&mut headers, "content-type", "text/html");
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0].1, "text/html");
    }

    #[test]
    fn test_guess_mime_types() {
        assert_eq!(
            guess_mime_type(Path::new("file.html")),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            guess_mime_type(Path::new("file.css")),
            "text/css; charset=utf-8"
        );
        assert_eq!(guess_mime_type(Path::new("file.png")), "image/png");
        assert_eq!(guess_mime_type(Path::new("file.ico")), "image/x-icon");
        assert_eq!(guess_mime_type(Path::new("file.ttf")), "font/ttf");
        assert_eq!(
            guess_mime_type(Path::new("file.xhtml")),
            "application/xhtml+xml; charset=utf-8"
        );
    }

    #[test]
    fn test_status_reason_phrase() {
        assert_eq!(status_reason_phrase(200), "OK");
        assert_eq!(status_reason_phrase(404), "Not Found");
        assert_eq!(status_reason_phrase(500), "Internal Server Error");
    }

    #[test]
    fn test_resolve_target_file_path_traversal_prevention() {
        let temp_dir = std::env::temp_dir().join("mango_traversal_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let secret_file = temp_dir.join("secret.txt");
        std::fs::write(&secret_file, "secret_data").unwrap();

        let sub_dir = temp_dir.join("sub");
        let _ = std::fs::create_dir_all(&sub_dir);
        let normal_file = sub_dir.join("test.html");
        std::fs::write(&normal_file, "<h1>OK</h1>").unwrap();

        // Direct path traversal attempts must be rejected with None (resulting in 404)
        assert_eq!(resolve_target_file(&sub_dir, "../secret.txt"), None);
        assert_eq!(resolve_target_file(&sub_dir, "../../secret.txt"), None);
        assert_eq!(
            resolve_target_file(&sub_dir, "acid3/../../secret.txt"),
            None
        );
        assert_eq!(resolve_target_file(&sub_dir, r"..\secret.txt"), None);
        assert_eq!(resolve_target_file(&sub_dir, ".."), None);
        assert_eq!(resolve_target_file(&sub_dir, "/etc/passwd"), None);

        // Legitimate file must still resolve
        assert_eq!(
            resolve_target_file(&sub_dir, "test.html"),
            Some(normal_file)
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_server_rejects_path_traversal_http_404() {
        let server = WptServer::new("tests/wpt").expect("Failed to start server");
        let port = server.port();
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .write_all(b"GET /../../../../../../etc/passwd HTTP/1.1\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut resp = Vec::new();
        stream.read_to_end(&mut resp).unwrap();
        let resp_str = String::from_utf8_lossy(&resp);
        assert!(
            resp_str.contains("404 Not Found"),
            "Must return 404 on path traversal attempt, got: {}",
            resp_str
        );
    }
}
