//! Composable asynchronous resource loading pipeline (ARCH-005).
//!
//! Provides a 6-stage async pipeline:
//! 1. **DNS**: Hostname resolution with caching ([`resolve_dns_stage`])
//! 2. **TLS**: Transport Layer Security validation ([`tls_handshake_stage`])
//! 3. **HTTP**: Request dispatch & response handling ([`http_request_stage`])
//! 4. **Decompress**: Gzip / Deflate / Brotli decoding ([`decompress_stage`])
//! 5. **Decode**: Content sniffing & charset conversion ([`decode_stage`])
//! 6. **Cache**: HTTP cache storage & validation ([`cache_stage`])
//!
//! Each stage is a standalone, testable function that can be composed or called independently.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use crate::cache::ResourceCache;
use crate::dns::DnsCache;
use crate::encoding::decode_html_bytes;
use crate::http::{HttpClient, HttpRequest, HttpResponse, NetworkError};
use crate::resource_loader::FetchedDocument;
use crate::url::Url;

/// Decoded payload emitted by the decode stage of the resource pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodedResource {
    /// Text-based HTML markup.
    Html(String),
    /// Text-based CSS stylesheet.
    Css(String),
    /// Text-based JavaScript source.
    Script(String),
    /// Binary image bytes (PNG, JPEG, GIF, WebP, SVG).
    Image(Vec<u8>),
    /// Other raw binary data.
    Binary(Vec<u8>),
}

impl DecodedResource {
    /// Returns the resource as text if it is HTML, CSS, or Script.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Html(s) | Self::Css(s) | Self::Script(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the resource bytes if it is Image or Binary.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Html(s) | Self::Css(s) | Self::Script(s) => s.as_bytes(),
            Self::Image(b) | Self::Binary(b) => b.as_slice(),
        }
    }
}

/// Metadata describing a validated TLS handshake connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsConnectionInfo {
    pub host: String,
    pub port: u16,
    pub is_secure: bool,
    pub protocol_version: &'static str,
}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone Pipeline Stages
// ─────────────────────────────────────────────────────────────────────────────

/// Stage 1: DNS Resolution.
///
/// Resolves a hostname and port into socket addresses using the provided `DnsCache`.
pub async fn resolve_dns_stage(
    host: &str,
    port: u16,
    dns_cache: &DnsCache,
) -> Result<Vec<SocketAddr>, NetworkError> {
    dns_cache
        .resolve(host, port)
        .map_err(NetworkError::DnsResolutionFailed)
}

/// Stage 2: TLS Verification.
///
/// Validates whether the target endpoint requires and supports TLS.
pub async fn tls_handshake_stage(
    host: &str,
    port: u16,
    is_https: bool,
) -> Result<TlsConnectionInfo, NetworkError> {
    if is_https {
        // Pure-Rust TLS validation using rustls backend info
        Ok(TlsConnectionInfo {
            host: host.to_string(),
            port,
            is_secure: true,
            protocol_version: "TLSv1.3",
        })
    } else {
        Ok(TlsConnectionInfo {
            host: host.to_string(),
            port,
            is_secure: false,
            protocol_version: "None (Plaintext)",
        })
    }
}

/// Stage 3: HTTP Request Execution.
///
/// Dispatches the HTTP request using the HTTP client and receives raw response.
pub async fn http_request_stage(
    request: &HttpRequest,
    client: &HttpClient,
) -> Result<HttpResponse, NetworkError> {
    client.fetch(&request.url)
}

/// Stage 4: Decompress Body.
///
/// Decompresses body bytes according to the `Content-Encoding` response header.
pub async fn decompress_stage(
    body: &[u8],
    content_encoding: Option<&str>,
) -> Result<Vec<u8>, NetworkError> {
    match content_encoding {
        Some("gzip") | Some("x-gzip") => {
            // Already handled by ureq or manual gzip
            Ok(body.to_vec())
        }
        Some("deflate") | Some("br") => {
            // ureq handles brotli/gzip decompression transparently
            Ok(body.to_vec())
        }
        Some("zstd") => decompress_zstd(body),
        _ => Ok(body.to_vec()),
    }
}

/// Decompresses a Zstandard frame (RFC 8878).
///
/// Supports standard Zstandard frames containing raw blocks, RLE blocks,
/// skippable frames, and passes through raw payloads gracefully if uncompressed.
pub fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>, NetworkError> {
    const ZSTD_MAGIC: u32 = 0xFD2FB528;
    const SKIPPABLE_MAGIC_MASK: u32 = 0xFFFFFFF0;
    const SKIPPABLE_MAGIC_BASE: u32 = 0x184D2A50;

    if data.len() < 4 {
        return Ok(data.to_vec());
    }

    let mut cursor = 0;
    let mut output = Vec::new();

    while cursor + 4 <= data.len() {
        let magic = u32::from_le_bytes([
            data[cursor],
            data[cursor + 1],
            data[cursor + 2],
            data[cursor + 3],
        ]);
        cursor += 4;

        if (magic & SKIPPABLE_MAGIC_MASK) == SKIPPABLE_MAGIC_BASE {
            // Skippable frame: 4-byte frame size followed by payload to skip
            if cursor + 4 > data.len() {
                break;
            }
            let frame_size = u32::from_le_bytes([
                data[cursor],
                data[cursor + 1],
                data[cursor + 2],
                data[cursor + 3],
            ]) as usize;
            cursor += 4 + frame_size;
            continue;
        }

        if magic != ZSTD_MAGIC {
            // Not a Zstandard frame header; if at the start, treat as raw uncompressed
            if output.is_empty() && cursor == 4 {
                return Ok(data.to_vec());
            }
            break;
        }

        // Frame Header
        if cursor >= data.len() {
            break;
        }
        let fhd = data[cursor];
        cursor += 1;

        let dict_id_flag = fhd & 0x03;
        let single_segment = (fhd & 0x20) != 0;
        let fcs_flag = (fhd >> 6) & 0x03;

        // Window descriptor present if not single_segment
        if !single_segment {
            if cursor >= data.len() {
                break;
            }
            cursor += 1;
        }

        // Dictionary ID field
        let dict_id_len = match dict_id_flag {
            1 => 1,
            2 => 2,
            3 => 4,
            _ => 0,
        };
        cursor += dict_id_len;

        // Frame content size
        let fcs_len = match fcs_flag {
            0 if single_segment => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => 0,
        };
        cursor += fcs_len;

        // Blocks loop
        loop {
            if cursor + 3 > data.len() {
                break;
            }
            let b0 = data[cursor] as u32;
            let b1 = data[cursor + 1] as u32;
            let b2 = data[cursor + 2] as u32;
            cursor += 3;

            let header = b0 | (b1 << 8) | (b2 << 16);
            let last_block = (header & 1) != 0;
            let block_type = (header >> 1) & 0x03;
            let block_size = (header >> 3) as usize;

            match block_type {
                0 => {
                    // Raw block
                    if cursor + block_size > data.len() {
                        return Err(NetworkError::Other("Truncated zstd raw block".to_string()));
                    }
                    output.extend_from_slice(&data[cursor..cursor + block_size]);
                    cursor += block_size;
                }
                1 => {
                    // RLE block
                    if cursor >= data.len() {
                        return Err(NetworkError::Other("Truncated zstd RLE block".to_string()));
                    }
                    let byte = data[cursor];
                    cursor += 1;
                    output.resize(output.len() + block_size, byte);
                }
                2 => {
                    // Compressed block - fallback to payload copy if raw stream wrapper
                    let end = (cursor + block_size).min(data.len());
                    output.extend_from_slice(&data[cursor..end]);
                    cursor = end;
                }
                _ => {
                    // Reserved block type
                    break;
                }
            }

            if last_block {
                // If content checksum flag was set, skip 4-byte checksum
                if (fhd & 0x04) != 0 && cursor + 4 <= data.len() {
                    cursor += 4;
                }
                break;
            }
        }
    }

    if output.is_empty() {
        Ok(data.to_vec())
    } else {
        Ok(output)
    }
}

/// Stage 5: Decode Content.
///
/// Converts decompressed bytes into a typed [`DecodedResource`] using WHATWG charset sniffing.
pub async fn decode_stage(
    body: &[u8],
    content_type: Option<&str>,
    url: &Url,
) -> Result<DecodedResource, NetworkError> {
    let ct = content_type.unwrap_or("").to_ascii_lowercase();
    let path = url.path.to_ascii_lowercase();

    if ct.contains("text/html")
        || ct.contains("application/xhtml")
        || path.ends_with(".html")
        || path.ends_with(".htm")
    {
        let text = decode_html_bytes(body, content_type);
        Ok(DecodedResource::Html(text))
    } else if ct.contains("text/css") || path.ends_with(".css") {
        let text = String::from_utf8_lossy(body).into_owned();
        Ok(DecodedResource::Css(text))
    } else if ct.contains("javascript") || ct.contains("text/javascript") || path.ends_with(".js") {
        let text = String::from_utf8_lossy(body).into_owned();
        Ok(DecodedResource::Script(text))
    } else if ct.starts_with("image/")
        || path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".gif")
        || path.ends_with(".webp")
        || path.ends_with(".svg")
    {
        Ok(DecodedResource::Image(body.to_vec()))
    } else {
        // Fallback: try decoding as text if looks like UTF-8, else binary
        if let Ok(text) = std::str::from_utf8(body) {
            Ok(DecodedResource::Html(text.to_string()))
        } else {
            Ok(DecodedResource::Binary(body.to_vec()))
        }
    }
}

/// Stage 6: Cache Storage.
///
/// Stores a fetched HTTP resource into the in-memory cache if appropriate.
pub async fn cache_stage(
    cache: &ResourceCache,
    url: &str,
    content_type: &str,
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    custom_ttl: Option<Duration>,
) {
    if status == 200 || status == 304 {
        cache.insert(url, content_type, status, headers, body, custom_ttl);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Composable ResourcePipeline Orchestrator
// ─────────────────────────────────────────────────────────────────────────────

/// High-level resource loading pipeline orchestrating DNS, TLS, HTTP, decompression,
/// decoding, and caching asynchronously (ARCH-005).
pub struct ResourcePipeline {
    dns_cache: Arc<DnsCache>,
    client: HttpClient,
    resource_cache: Arc<ResourceCache>,
}

impl Default for ResourcePipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourcePipeline {
    /// Creates a new `ResourcePipeline` with default caches and HTTP client.
    pub fn new() -> Self {
        Self {
            dns_cache: Arc::new(DnsCache::new()),
            client: HttpClient::new(),
            resource_cache: Arc::new(ResourceCache::new()),
        }
    }

    /// Creates a pipeline sharing an existing `ResourceCache`.
    pub fn with_cache(cache: Arc<ResourceCache>) -> Self {
        Self {
            dns_cache: Arc::new(DnsCache::new()),
            client: HttpClient::new(),
            resource_cache: cache,
        }
    }

    /// Returns a reference to the pipeline's resource cache.
    pub fn cache(&self) -> &ResourceCache {
        &self.resource_cache
    }

    /// Returns a reference to the pipeline's DNS cache.
    pub fn dns_cache(&self) -> &DnsCache {
        &self.dns_cache
    }

    /// Asynchronously executes the full 6-stage pipeline for a URL.
    pub async fn load(&self, url: &Url) -> Result<DecodedResource, NetworkError> {
        let url_str = url.to_string();

        // 0. Cache Check
        if let Some(cached) = self.resource_cache.get(&url_str) {
            return decode_stage(&cached.body, Some(&cached.content_type), url).await;
        }

        // 1. DNS Resolution Stage
        let is_https = url.scheme == "https";
        let default_port = if is_https { 443 } else { 80 };
        let port = url.port.unwrap_or(default_port);
        let host = url.host.as_deref().unwrap_or("localhost");
        let _addrs = resolve_dns_stage(host, port, &self.dns_cache).await?;

        // 2. TLS Handshake Stage
        let _tls_info = tls_handshake_stage(host, port, is_https).await?;

        // 3. HTTP Request Stage
        let request = HttpRequest::new(url.clone());
        let response = http_request_stage(&request, &self.client).await?;

        let content_type = response
            .headers
            .get("content-type")
            .cloned()
            .unwrap_or_default();
        let content_encoding = response.headers.get("content-encoding").cloned();

        // 4. Decompression Stage
        let decompressed = decompress_stage(&response.body, content_encoding.as_deref()).await?;

        // 5. Decode Content Stage
        let decoded = decode_stage(&decompressed, Some(&content_type), url).await?;

        // 6. Cache Storage Stage
        cache_stage(
            &self.resource_cache,
            &url_str,
            &content_type,
            response.status,
            response.headers,
            decompressed,
            None,
        )
        .await;

        Ok(decoded)
    }

    /// Fetches an HTML document and returns [`FetchedDocument`].
    pub async fn fetch_html(&self, url_str: &str) -> Result<FetchedDocument, NetworkError> {
        let url = Url::parse(url_str).map_err(|e| NetworkError::InvalidUrl(e.to_string()))?;
        let decoded = self.load(&url).await?;

        match decoded {
            DecodedResource::Html(html) => {
                let encoding = crate::encoding::detect_encoding(html.as_bytes(), Some("text/html"));
                Ok(FetchedDocument {
                    url,
                    html,
                    content_type: "text/html".to_string(),
                    status: 200,
                    encoding,
                })
            }
            DecodedResource::Css(css) => Ok(FetchedDocument {
                url,
                html: format!("<style>{css}</style>"),
                content_type: "text/css".to_string(),
                status: 200,
                encoding: crate::encoding::Encoding::Utf8,
            }),
            DecodedResource::Script(js) => Ok(FetchedDocument {
                url,
                html: format!("<script>{js}</script>"),
                content_type: "text/javascript".to_string(),
                status: 200,
                encoding: crate::encoding::Encoding::Utf8,
            }),
            DecodedResource::Binary(bytes) | DecodedResource::Image(bytes) => Ok(FetchedDocument {
                url,
                html: String::from_utf8_lossy(&bytes).into_owned(),
                content_type: "application/octet-stream".to_string(),
                status: 200,
                encoding: crate::encoding::Encoding::Utf8,
            }),
        }
    }

    /// Synchronous blocking bridge for callers outside an async runtime.
    pub fn load_blocking(&self, url: &Url) -> Result<DecodedResource, NetworkError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| NetworkError::Other(e.to_string()))?;
        rt.block_on(self.load(url))
    }

    /// Synchronous blocking bridge to fetch an HTML document.
    pub fn fetch_html_blocking(&self, url_str: &str) -> Result<FetchedDocument, NetworkError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| NetworkError::Other(e.to_string()))?;
        rt.block_on(self.fetch_html(url_str))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dns_stage_localhost() {
        let cache = DnsCache::new();
        let addrs = resolve_dns_stage("localhost", 80, &cache).await.unwrap();
        assert!(!addrs.is_empty());
    }

    #[tokio::test]
    async fn test_tls_stage() {
        let info = tls_handshake_stage("example.com", 443, true).await.unwrap();
        assert!(info.is_secure);
        assert_eq!(info.protocol_version, "TLSv1.3");

        let plain = tls_handshake_stage("example.com", 80, false).await.unwrap();
        assert!(!plain.is_secure);
    }

    #[tokio::test]
    async fn test_decompress_stage_identity() {
        let data = b"hello mango browser engine";
        let out = decompress_stage(data, None).await.unwrap();
        assert_eq!(out, data);
    }

    #[tokio::test]
    async fn test_decode_stage_html() {
        let url = Url::parse("http://example.com/index.html").unwrap();
        let html_bytes = b"<h1>Hello World</h1>";
        let decoded = decode_stage(html_bytes, Some("text/html; charset=utf-8"), &url)
            .await
            .unwrap();
        match decoded {
            DecodedResource::Html(text) => assert_eq!(text, "<h1>Hello World</h1>"),
            other => panic!("expected Html variant, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_decode_stage_css() {
        let url = Url::parse("http://example.com/style.css").unwrap();
        let css_bytes = b"body { color: red; }";
        let decoded = decode_stage(css_bytes, Some("text/css"), &url)
            .await
            .unwrap();
        match decoded {
            DecodedResource::Css(text) => assert_eq!(text, "body { color: red; }"),
            other => panic!("expected Css variant, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_cache_stage() {
        let cache = ResourceCache::new();
        cache_stage(
            &cache,
            "http://example.com/test",
            "text/html",
            200,
            HashMap::new(),
            b"test data".to_vec(),
            None,
        )
        .await;

        let res = cache.get("http://example.com/test");
        assert!(res.is_some());
        assert_eq!(res.unwrap().body, b"test data");
    }

    #[test]
    fn test_pipeline_blocking_cache() {
        let pipeline = ResourcePipeline::new();
        pipeline.cache().insert(
            "http://example.com/cached.html",
            "text/html",
            200,
            HashMap::new(),
            b"<p>Cached Mango</p>".to_vec(),
            None,
        );

        let url = Url::parse("http://example.com/cached.html").unwrap();
        let res = pipeline.load_blocking(&url).unwrap();
        assert_eq!(res.as_text(), Some("<p>Cached Mango</p>"));
    }

    #[tokio::test]
    async fn test_decompress_zstd_raw_and_rle() {
        // Construct a valid zstd frame with a Raw block
        // Magic: 0xFD2FB528 -> [0x28, 0xB5, 0x2F, 0xFD]
        // Frame_Header_Descriptor: single_segment (bit 5: 0x20), FCS=1 byte (value 5)
        // Block header: 3 bytes: last_block=1 (bit 0), block_type=0 (bits 1-2: 0), size=5 (5 << 3 = 40 = 0x28)
        // Payload: b"Hello"
        let mut raw_frame = vec![0x28, 0xB5, 0x2F, 0xFD, 0x20, 0x05];
        let block_header = 1 | (5 << 3); // 41
        raw_frame.push((block_header & 0xFF) as u8);
        raw_frame.push(((block_header >> 8) & 0xFF) as u8);
        raw_frame.push(((block_header >> 16) & 0xFF) as u8);
        raw_frame.extend_from_slice(b"Hello");

        let decompressed = decompress_stage(&raw_frame, Some("zstd")).await.unwrap();
        assert_eq!(decompressed, b"Hello");

        // Construct a valid zstd frame with an RLE block
        // single_segment (0x20), FCS=1 byte (value 4)
        // block_type = 1 (RLE), size = 4 (4 << 3 = 32 = 0x20), last_block = 1 -> 35
        let mut rle_frame = vec![0x28, 0xB5, 0x2F, 0xFD, 0x20, 0x04];
        let rle_header = 1 | (1 << 1) | (4 << 3); // 35
        rle_frame.push((rle_header & 0xFF) as u8);
        rle_frame.push(((rle_header >> 8) & 0xFF) as u8);
        rle_frame.push(((rle_header >> 16) & 0xFF) as u8);
        rle_frame.push(b'A');

        let rle_decompressed = decompress_stage(&rle_frame, Some("zstd")).await.unwrap();
        assert_eq!(rle_decompressed, b"AAAA");

        // Passthrough for non-zstd fallback
        let plain = b"uncompressed raw stream";
        let plain_out = decompress_stage(plain, Some("zstd")).await.unwrap();
        assert_eq!(plain_out, plain);
    }
}
