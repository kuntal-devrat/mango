//! Static HTTP server for running canonical Web Platform Tests (WPT) locally.
//!
//! Provides a local HTTP/1.1 server serving the WPT test suite with proper CORS,
//! MIME types, and resources mapping (/resources/testharness.js, etc.).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
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

    // Strip query string and fragment
    let path_no_query = full_path.split('?').next().unwrap_or(full_path);
    let path_no_hash = path_no_query.split('#').next().unwrap_or(path_no_query);

    // Special WPT aliases:
    // /resources/testharnessreport.js -> Mango's reporter hook
    let clean_path = path_no_hash.trim_start_matches('/');
    let target_file = if path_no_hash == "/resources/testharnessreport.js" {
        base_dir.join("testharnessreport.js")
    } else {
        let in_upstream = base_dir.join("upstream").join(clean_path);
        if in_upstream.is_file() {
            in_upstream
        } else {
            base_dir.join(clean_path)
        }
    };

    if target_file.is_file()
        && let Ok(bytes) = std::fs::read(&target_file)
    {
        let mime = guess_mime_type(&target_file);
        let header = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\
             Access-Control-Allow-Origin: *\r\n\
             Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
             Access-Control-Allow-Headers: *\r\n\
             Timing-Allow-Origin: *\r\n\
             Connection: close\r\n\r\n",
            mime,
            bytes.len()
        );
        stream.write_all(header.as_bytes())?;
        if method != "HEAD" {
            stream.write_all(&bytes)?;
        }
        return Ok(());
    }

    // 404 Not Found fallback
    let not_found_body = format!("404 Not Found: {}", path_no_hash);
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
        _ => "application/octet-stream",
    }
}
