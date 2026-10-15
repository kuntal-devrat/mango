//! Pure-Rust RFC 6455 WebSocket Client Implementation.
//!
//! Provides a standards-compliant WebSocket client supporting text and binary frames,
//! masked client frames, ping-pong keepalive, and graceful close handshakes.
//! All network I/O executes off the main thread to ensure zero UI freezes.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::url::Url;

/// Maximum WebSocket frame payload (16 MiB) to prevent OOM on malicious length prefix.
pub const MAX_WS_PAYLOAD: usize = 16 * 1024 * 1024;
/// Maximum handshake header bytes.
const MAX_HANDSHAKE_BYTES: usize = 64 * 1024;

// ── Unified Read+Write trait for plain TCP and TLS streams (G-03) ─────────────

/// A trait combining `Read + Write + Send` so `run_websocket_client` can hold
/// either a plain `TcpStream` or a TLS-wrapped stream behind the same pointer.
trait ReadWrite: Read + Write + Send {}
impl<T: Read + Write + Send> ReadWrite for T {}

/// Wraps a `TcpStream` in a rustls client session.
///
/// Uses the same `ureq`-compatible rustls+webpki root store so TLS certificates
/// are validated by the same engine as regular HTTPS navigation.
fn make_tls_stream(tcp: TcpStream, hostname: &str) -> Result<impl Read + Write + Send, String> {
    use std::sync::Arc;

    // Build a rustls client configuration with the system webpki root store.
    let root_store = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let connector = rustls::ClientConnection::new(
        Arc::new(config),
        rustls::pki_types::ServerName::try_from(hostname.to_string())
            .map_err(|e| format!("Invalid TLS server name '{hostname}': {e}"))?,
    )
    .map_err(|e| format!("rustls init error: {e}"))?;

    Ok(rustls::StreamOwned::new(connector, tcp))
}

/// Generates 16 cryptographically random bytes for the handshake nonce.
/// Reads from the OS CSPRNG (`/dev/urandom`); falls back to a
/// time/pid/counter mix when unavailable.
pub fn random_bytes_16() -> [u8; 16] {
    let mut out = [0u8; 16];
    if let Ok(f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read as _;
        let mut f = f;
        let mut buf = [0u8; 16];
        if f.read_exact(&mut buf).is_ok() {
            return buf;
        }
    }
    // Fallback: mix nanotime, pid, and an atomic counter through xorshift.
    static COUNTER: AtomicU64 = AtomicU64::new(0x9E3779B97F4A7C15);
    let c = COUNTER.fetch_add(0x9E3779B97F4A7C15, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x12345678);
    let pid = std::process::id() as u64;
    let tid = {
        let s = format!("{:?}", std::thread::current().id());
        let mut h: u64 = 0xcbf29ce484222325;
        for b in s.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    };
    let mut state = nanos ^ pid.wrapping_mul(0x9E3779B1) ^ tid ^ c;
    // xorshift64* to fill 16 bytes.
    for chunk in out.chunks_mut(8) {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let v = state.wrapping_mul(0x2545F4914F6CDD1D);
        chunk.copy_from_slice(&v.to_le_bytes()[..chunk.len()]);
    }
    // Mix counter in to guarantee uniqueness across rapid calls.
    for (i, b) in out.iter_mut().enumerate() {
        *b ^= ((c >> ((i % 8) * 8)) & 0xFF) as u8;
    }
    out
}

/// Generates a random 4-byte masking key per frame (RFC 6455 §5.3).
pub fn random_mask() -> [u8; 4] {
    let b = random_bytes_16();
    [b[0], b[1], b[2], b[3]]
}

/// Computes the SHA-1 digest (RFC 3174) for the WebSocket handshake key.
pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    let ml = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&ml.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

/// Standard Base64 encoder helper for handshake key and accept tokens.
pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i];
        let b1 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] } else { 0 };

        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);

        if i + 1 < data.len() {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }

        if i + 2 < data.len() {
            out.push(TABLE[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

/// Computes the expected `Sec-WebSocket-Accept` value given the client key.
pub fn compute_accept_key(client_key: &str) -> String {
    const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let combined = format!("{}{}", client_key.trim(), GUID);
    let digest = sha1(combined.as_bytes());
    base64_encode(&digest)
}

/// WebSocket frame opcodes (RFC 6455 §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl Opcode {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x0 => Some(Self::Continuation),
            0x1 => Some(Self::Text),
            0x2 => Some(Self::Binary),
            0x8 => Some(Self::Close),
            0x9 => Some(Self::Ping),
            0xA => Some(Self::Pong),
            _ => None,
        }
    }
}

/// Encodes an RFC 6455 client frame (always masked, per RFC 6455 §5.3).
pub fn encode_client_frame(opcode: Opcode, payload: &[u8], mask: [u8; 4]) -> Vec<u8> {
    let mut frame = Vec::new();
    // Byte 0: FIN = 1 | RSV = 0 | Opcode
    frame.push(0x80 | (opcode as u8));

    // Byte 1: Mask = 1 | Payload Len
    let len = payload.len();
    if len <= 125 {
        frame.push(0x80 | (len as u8));
    } else if len <= 65535 {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    // Mask key (4 bytes)
    frame.extend_from_slice(&mask);

    // Masked payload
    for (i, &byte) in payload.iter().enumerate() {
        frame.push(byte ^ mask[i % 4]);
    }

    frame
}

/// Decodes a single RFC 6455 frame from a reader.
///
/// Enforces: RSV bits must be 0, minimal length encoding, control frames
/// (`Close`/`Ping`/`Pong`) must have payload ≤125 and FIN=1, payload capped
/// at [`MAX_WS_PAYLOAD`], and `Text` payloads must be valid UTF-8.
pub fn decode_frame<R: Read>(reader: &mut R) -> std::io::Result<(Opcode, Vec<u8>, bool)> {
    let mut header = [0u8; 2];
    reader.read_exact(&mut header)?;

    let fin = (header[0] & 0x80) != 0;
    // RSV1/RSV2/RSV3 must be 0 (no extensions negotiated).
    if (header[0] & 0x70) != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "RSV bits must be 0 without negotiated extensions",
        ));
    }
    let opcode_byte = header[0] & 0x0F;
    let opcode = Opcode::from_u8(opcode_byte)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Unknown opcode"))?;

    let masked = (header[1] & 0x80) != 0;
    let len_code = header[1] & 0x7F;

    let payload_len: usize = if len_code <= 125 {
        len_code as usize
    } else if len_code == 126 {
        let mut ext = [0u8; 2];
        reader.read_exact(&mut ext)?;
        let v = u16::from_be_bytes(ext) as usize;
        // Minimal-length encoding: 126 must mean >125.
        if v <= 125 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Non-minimal frame length encoding (126 for len <= 125)",
            ));
        }
        v
    } else {
        let mut ext = [0u8; 8];
        reader.read_exact(&mut ext)?;
        // MSB must be 0 per RFC 6455 §5.2.
        if (ext[0] & 0x80) != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Frame length MSB must be 0",
            ));
        }
        let v = u64::from_be_bytes(ext);
        if v <= 65535 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Non-minimal frame length encoding (127 for len <= 65535)",
            ));
        }
        if v > MAX_WS_PAYLOAD as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WebSocket frame exceeds 16MiB cap",
            ));
        }
        v as usize
    };

    // Control frames must be ≤125 bytes and unfragmented.
    if matches!(opcode, Opcode::Close | Opcode::Ping | Opcode::Pong) {
        if !fin {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Fragmented control frame",
            ));
        }
        if payload_len > 125 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Control frame payload exceeds 125 bytes",
            ));
        }
    }

    if payload_len > MAX_WS_PAYLOAD {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WebSocket frame exceeds 16MiB cap",
        ));
    }

    let mask_key = if masked {
        let mut m = [0u8; 4];
        reader.read_exact(&mut m)?;
        Some(m)
    } else {
        None
    };

    let mut payload = vec![0u8; payload_len];
    reader.read_exact(&mut payload)?;

    if let Some(mask) = mask_key {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[i % 4];
        }
    }

    // Text frames must be valid UTF-8; fail the connection otherwise.
    if opcode == Opcode::Text && std::str::from_utf8(&payload).is_err() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid UTF-8 in text frame",
        ));
    }
    // Close reason (bytes 2..) must be valid UTF-8 per RFC 6455 §5.5.1.
    if opcode == Opcode::Close && payload.len() > 2 && std::str::from_utf8(&payload[2..]).is_err() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid UTF-8 in close reason",
        ));
    }

    Ok((opcode, payload, fin))
}

/// Events dispatched from a WebSocket connection to the host runtime.
#[derive(Debug, Clone)]
pub enum WebSocketEvent {
    Open {
        protocol: String,
    },
    Message {
        data: String,
    },
    BinaryMessage {
        data: Vec<u8>,
    },
    Error {
        message: String,
    },
    Close {
        code: u16,
        reason: String,
        was_clean: bool,
    },
}

/// Internal commands sent to the WebSocket background thread.
pub enum WebSocketCommand {
    SendText(String),
    SendBinary(Vec<u8>),
    Close { code: u16, reason: String },
}

/// A handle to an active background WebSocket connection.
pub struct WebSocketConnection {
    pub id: u64,
    cmd_tx: Sender<WebSocketCommand>,
    _thread: Option<JoinHandle<()>>,
}

impl WebSocketConnection {
    pub fn send_text(&self, text: String) -> Result<(), String> {
        self.cmd_tx
            .send(WebSocketCommand::SendText(text))
            .map_err(|e| format!("Failed to send text: {e}"))
    }

    pub fn send_binary(&self, data: Vec<u8>) -> Result<(), String> {
        self.cmd_tx
            .send(WebSocketCommand::SendBinary(data))
            .map_err(|e| format!("Failed to send binary: {e}"))
    }

    pub fn close(&self, code: u16, reason: String) -> Result<(), String> {
        self.cmd_tx
            .send(WebSocketCommand::Close { code, reason })
            .map_err(|e| format!("Failed to send close: {e}"))
    }
}

/// Global or engine-level manager for active WebSocket connections.
#[derive(Clone, Default)]
pub struct WebSocketManager {
    connections: Arc<Mutex<HashMap<u64, WebSocketConnection>>>,
    next_id: Arc<AtomicU64>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Spawns a background worker thread to connect and manage a WebSocket session.
    pub fn connect<F>(&self, id: u64, url_str: &str, protocols: &str, event_callback: F)
    where
        F: Fn(WebSocketEvent) + Send + Sync + 'static,
    {
        let (cmd_tx, cmd_rx) = mpsc::channel::<WebSocketCommand>();
        let target_url = url_str.to_string();
        let target_protocols = protocols.to_string();
        let cb = Arc::new(event_callback);

        let thread_handle = thread::Builder::new()
            .name(format!("mango-ws-{}", id))
            .spawn(move || {
                run_websocket_client(target_url, target_protocols, cmd_rx, cb);
            })
            .ok();

        let conn = WebSocketConnection {
            id,
            cmd_tx,
            _thread: thread_handle,
        };

        if let Ok(mut conns) = self.connections.lock() {
            conns.insert(id, conn);
        }
    }

    pub fn send_text(&self, id: u64, text: String) -> Result<(), String> {
        let conns = self.connections.lock().map_err(|e| e.to_string())?;
        if let Some(conn) = conns.get(&id) {
            conn.send_text(text)
        } else {
            Err("WebSocket connection not found".to_string())
        }
    }

    pub fn close(&self, id: u64, code: u16, reason: String) -> Result<(), String> {
        let mut conns = self.connections.lock().map_err(|e| e.to_string())?;
        if let Some(conn) = conns.remove(&id) {
            conn.close(code, reason)
        } else {
            Ok(())
        }
    }
}

/// Background worker loop managing the TCP stream and RFC 6455 protocol states.
fn run_websocket_client<F>(
    url_str: String,
    protocols: String,
    cmd_rx: Receiver<WebSocketCommand>,
    cb: Arc<F>,
) where
    F: Fn(WebSocketEvent) + Send + Sync + 'static,
{
    let parsed_url = match Url::parse(&url_str) {
        Ok(u) => u,
        Err(e) => {
            cb(WebSocketEvent::Error {
                message: format!("Invalid WebSocket URL '{}': {e}", url_str),
            });
            cb(WebSocketEvent::Close {
                code: 1006,
                reason: "Invalid URL".to_string(),
                was_clean: false,
            });
            return;
        }
    };

    let scheme = parsed_url.scheme.to_lowercase();
    if scheme != "ws" && scheme != "wss" {
        cb(WebSocketEvent::Error {
            message: format!("Unsupported WebSocket scheme: '{}'", scheme),
        });
        cb(WebSocketEvent::Close {
            code: 1006,
            reason: "Unsupported scheme".to_string(),
            was_clean: false,
        });
        return;
    }

    let host = match parsed_url.host.as_deref() {
        Some(h) if !h.is_empty() => h,
        _ => {
            cb(WebSocketEvent::Error {
                message: "Missing WebSocket host".to_string(),
            });
            cb(WebSocketEvent::Close {
                code: 1006,
                reason: "Missing host".to_string(),
                was_clean: false,
            });
            return;
        }
    };

    let port = parsed_url
        .port
        .unwrap_or(if scheme == "wss" { 443 } else { 80 });
    let use_tls = scheme == "wss";
    let mut path = if parsed_url.path.is_empty() {
        "/".to_string()
    } else {
        parsed_url.path.clone()
    };
    if let Some(q) = &parsed_url.query {
        path.push('?');
        path.push_str(q);
    }
    let host_owned = host.to_string();

    // Resolve hostname via DNS (ToSocketAddrs), never parse as a literal SocketAddr.
    // A resolution failure is a hard error — never fall back to 127.0.0.1.
    let host_port = format!("{}:{}", host_owned, port);
    let addrs: Vec<SocketAddr> = match host_port.to_socket_addrs() {
        Ok(iter) => iter.collect(),
        Err(e) => {
            cb(WebSocketEvent::Error {
                message: format!("DNS resolution failed for {host_owned}: {e}"),
            });
            cb(WebSocketEvent::Close {
                code: 1006,
                reason: "DNS resolution failed".to_string(),
                was_clean: false,
            });
            return;
        }
    };
    if addrs.is_empty() {
        cb(WebSocketEvent::Error {
            message: format!("DNS resolution failed for {host_owned}: no addresses"),
        });
        cb(WebSocketEvent::Close {
            code: 1006,
            reason: "DNS resolution failed".to_string(),
            was_clean: false,
        });
        return;
    }

    // Attempt TCP connection with 5s timeout, trying each resolved address.
    let mut tcp_stream: Option<TcpStream> = None;
    let mut last_err = String::new();
    for addr in &addrs {
        match TcpStream::connect_timeout(addr, Duration::from_secs(5)) {
            Ok(s) => {
                tcp_stream = Some(s);
                break;
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }
    let tcp_stream = match tcp_stream {
        Some(s) => s,
        None => {
            cb(WebSocketEvent::Error {
                message: format!("Failed to connect to {host_port}: {last_err}"),
            });
            cb(WebSocketEvent::Close {
                code: 1006,
                reason: "Connection failed".to_string(),
                was_clean: false,
            });
            return;
        }
    };

    let _ = tcp_stream.set_nodelay(true);
    let _ = tcp_stream.set_read_timeout(Some(Duration::from_millis(100)));

    // For wss:// wrap the TCP stream in a rustls TLS session.
    // We use a Box<dyn ReadWrite> to unify the plain and TLS paths. (G-03 fix)
    let mut stream: Box<dyn ReadWrite> = if use_tls {
        match make_tls_stream(tcp_stream, &host_owned) {
            Ok(s) => Box::new(s),
            Err(e) => {
                cb(WebSocketEvent::Error {
                    message: format!("TLS handshake failed for {}: {}", host_owned, e),
                });
                cb(WebSocketEvent::Close {
                    code: 1006,
                    reason: "TLS handshake failed".to_string(),
                    was_clean: false,
                });
                return;
            }
        }
    } else {
        Box::new(tcp_stream)
    };

    // Generate random 16-byte client nonce per connection.
    let nonce: [u8; 16] = random_bytes_16();
    let client_key = base64_encode(&nonce);

    let mut handshake_req = format!(
        "GET {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {}\r\n\
         Sec-WebSocket-Version: 13\r\n",
        path, host_owned, client_key
    );
    if !protocols.is_empty() {
        handshake_req.push_str(&format!("Sec-WebSocket-Protocol: {}\r\n", protocols));
    }
    handshake_req.push_str("\r\n");

    if let Err(e) = stream.write_all(handshake_req.as_bytes()) {
        cb(WebSocketEvent::Error {
            message: format!("Failed to send handshake: {}", e),
        });
        cb(WebSocketEvent::Close {
            code: 1006,
            reason: "Handshake write failed".to_string(),
            was_clean: false,
        });
        return;
    }

    // Read handshake response (capped to prevent DoS).
    let mut response_buf = Vec::new();
    let mut byte = [0u8; 1];
    let mut header_complete = false;
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < Duration::from_secs(5) {
        match stream.read(&mut byte) {
            Ok(1) => {
                response_buf.push(byte[0]);
                if response_buf.len() > MAX_HANDSHAKE_BYTES {
                    break;
                }
                if response_buf.ends_with(b"\r\n\r\n") {
                    header_complete = true;
                    break;
                }
            }
            Ok(_) => thread::sleep(Duration::from_millis(10)),
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }

    if !header_complete {
        cb(WebSocketEvent::Error {
            message: "Handshake response timeout or incomplete headers".to_string(),
        });
        cb(WebSocketEvent::Close {
            code: 1006,
            reason: "Handshake timed out".to_string(),
            was_clean: false,
        });
        return;
    }

    let resp_str = String::from_utf8_lossy(&response_buf);
    if !resp_str.contains("101 Switching Protocols") {
        cb(WebSocketEvent::Error {
            message: format!(
                "Server rejected WebSocket upgrade: {}",
                resp_str.lines().next().unwrap_or("")
            ),
        });
        cb(WebSocketEvent::Close {
            code: 1006,
            reason: "Upgrade rejected".to_string(),
            was_clean: false,
        });
        return;
    }

    // Verify Sec-WebSocket-Accept to prevent response-smuggling/proxy confusion.
    let expected_accept = compute_accept_key(&client_key);
    let server_accept = resp_str
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("sec-websocket-accept"))
        .map(|(_, v)| v.trim().to_string());
    match server_accept {
        Some(v) if v == expected_accept => {}
        Some(v) => {
            cb(WebSocketEvent::Error {
                message: format!(
                    "WebSocket accept-key mismatch (expected {expected_accept}, got {v})"
                ),
            });
            cb(WebSocketEvent::Close {
                code: 1006,
                reason: "Accept key mismatch".to_string(),
                was_clean: false,
            });
            return;
        }
        None => {
            cb(WebSocketEvent::Error {
                message: "Missing Sec-WebSocket-Accept in handshake response".to_string(),
            });
            cb(WebSocketEvent::Close {
                code: 1006,
                reason: "Missing accept key".to_string(),
                was_clean: false,
            });
            return;
        }
    }

    // Handshake successful!
    let negotiated_protocol = if !protocols.is_empty() {
        protocols.split(',').next().unwrap_or("").trim().to_string()
    } else {
        String::new()
    };
    cb(WebSocketEvent::Open {
        protocol: negotiated_protocol,
    });

    let running = Arc::new(AtomicBool::new(true));
    // Fragment reassembly buffer: (initial opcode, accumulated payload).
    let mut frag_opcode: Option<Opcode> = None;
    let mut frag_buffer: Vec<u8> = Vec::new();

    // Bi-directional event loop
    while running.load(Ordering::Relaxed) {
        // Drain any outgoing commands from JS
        loop {
            match cmd_rx.try_recv() {
                Ok(cmd) => {
                    let mask = random_mask();
                    match cmd {
                        WebSocketCommand::SendText(text) => {
                            let frame = encode_client_frame(Opcode::Text, text.as_bytes(), mask);
                            if let Err(e) = stream.write_all(&frame) {
                                cb(WebSocketEvent::Error {
                                    message: format!("Socket write error: {e}"),
                                });
                                running.store(false, Ordering::Relaxed);
                                break;
                            }
                        }
                        WebSocketCommand::SendBinary(data) => {
                            let frame = encode_client_frame(Opcode::Binary, &data, mask);
                            if let Err(e) = stream.write_all(&frame) {
                                cb(WebSocketEvent::Error {
                                    message: format!("Socket write error: {e}"),
                                });
                                running.store(false, Ordering::Relaxed);
                                break;
                            }
                        }
                        WebSocketCommand::Close { code, reason } => {
                            let mut payload = code.to_be_bytes().to_vec();
                            payload.extend_from_slice(reason.as_bytes());
                            let frame = encode_client_frame(Opcode::Close, &payload, mask);
                            let _ = stream.write_all(&frame);
                            cb(WebSocketEvent::Close {
                                code,
                                reason,
                                was_clean: true,
                            });
                            running.store(false, Ordering::Relaxed);
                            return;
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    running.store(false, Ordering::Relaxed);
                    break;
                }
            }
        }

        if !running.load(Ordering::Relaxed) {
            break;
        }

        // Try reading frame from stream (with Continuation reassembly).
        match decode_frame(&mut stream) {
            Ok((Opcode::Text, payload, fin)) => {
                if frag_opcode.is_some() {
                    cb(WebSocketEvent::Error {
                        message: "Protocol error: new data frame while fragmented message pending"
                            .to_string(),
                    });
                    cb(WebSocketEvent::Close {
                        code: 1002,
                        reason: "Protocol error".to_string(),
                        was_clean: false,
                    });
                    break;
                }
                if fin {
                    // decode_frame already validated UTF-8.
                    let msg = String::from_utf8(payload).unwrap_or_default();
                    cb(WebSocketEvent::Message { data: msg });
                } else {
                    if payload.len() > MAX_WS_PAYLOAD {
                        cb(WebSocketEvent::Error {
                            message: "Fragment exceeds cap".to_string(),
                        });
                        break;
                    }
                    frag_opcode = Some(Opcode::Text);
                    frag_buffer = payload;
                }
            }
            Ok((Opcode::Binary, payload, fin)) => {
                if frag_opcode.is_some() {
                    cb(WebSocketEvent::Error {
                        message: "Protocol error: new data frame while fragmented message pending"
                            .to_string(),
                    });
                    cb(WebSocketEvent::Close {
                        code: 1002,
                        reason: "Protocol error".to_string(),
                        was_clean: false,
                    });
                    break;
                }
                if fin {
                    cb(WebSocketEvent::BinaryMessage { data: payload });
                } else {
                    if payload.len() > MAX_WS_PAYLOAD {
                        cb(WebSocketEvent::Error {
                            message: "Fragment exceeds cap".to_string(),
                        });
                        break;
                    }
                    frag_opcode = Some(Opcode::Binary);
                    frag_buffer = payload;
                }
            }
            Ok((Opcode::Continuation, payload, fin)) => {
                let Some(start) = frag_opcode else {
                    cb(WebSocketEvent::Error {
                        message: "Protocol error: unexpected continuation frame".to_string(),
                    });
                    cb(WebSocketEvent::Close {
                        code: 1002,
                        reason: "Protocol error".to_string(),
                        was_clean: false,
                    });
                    break;
                };
                if frag_buffer.len().saturating_add(payload.len()) > MAX_WS_PAYLOAD {
                    cb(WebSocketEvent::Error {
                        message: "Reassembled message exceeds 16MiB cap".to_string(),
                    });
                    cb(WebSocketEvent::Close {
                        code: 1009,
                        reason: "Message too big".to_string(),
                        was_clean: false,
                    });
                    break;
                }
                frag_buffer.extend_from_slice(&payload);
                if fin {
                    match start {
                        Opcode::Text => match String::from_utf8(std::mem::take(&mut frag_buffer)) {
                            Ok(msg) => cb(WebSocketEvent::Message { data: msg }),
                            Err(_) => {
                                cb(WebSocketEvent::Error {
                                    message: "Invalid UTF-8 in fragmented text".to_string(),
                                });
                                cb(WebSocketEvent::Close {
                                    code: 1007,
                                    reason: "Invalid UTF-8".to_string(),
                                    was_clean: false,
                                });
                                break;
                            }
                        },
                        _ => {
                            cb(WebSocketEvent::BinaryMessage {
                                data: std::mem::take(&mut frag_buffer),
                            });
                        }
                    }
                    frag_opcode = None;
                }
            }
            Ok((Opcode::Ping, payload, _fin)) => {
                // Reply with Pong
                let pong_frame = encode_client_frame(Opcode::Pong, &payload, random_mask());
                let _ = stream.write_all(&pong_frame);
            }
            Ok((Opcode::Close, payload, _fin)) => {
                let code = if payload.len() >= 2 {
                    u16::from_be_bytes([payload[0], payload[1]])
                } else {
                    1000
                };
                let reason = if payload.len() > 2 {
                    String::from_utf8_lossy(&payload[2..]).to_string()
                } else {
                    String::new()
                };
                // Reply with close frame if not already sent
                let close_reply = encode_client_frame(Opcode::Close, &payload, random_mask());
                let _ = stream.write_all(&close_reply);
                cb(WebSocketEvent::Close {
                    code,
                    reason,
                    was_clean: true,
                });
                break;
            }
            Ok((Opcode::Pong, _, _)) => {}
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                thread::sleep(Duration::from_millis(5));
            }
            Err(e) => {
                cb(WebSocketEvent::Error {
                    message: format!("Socket read error: {e}"),
                });
                cb(WebSocketEvent::Close {
                    code: 1006,
                    reason: "Connection terminated abnormally".to_string(),
                    was_clean: false,
                });
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha1_and_base64() {
        let digest = sha1(b"The quick brown fox jumps over the lazy dog");
        let hex = digest
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        assert_eq!(hex, "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12");

        let empty = sha1(b"");
        let empty_hex = empty
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        assert_eq!(empty_hex, "da39a3ee5e6b4b0d3255bfef95601890afd80709");

        let b64 = base64_encode(b"Hello world");
        assert_eq!(b64, "SGVsbG8gd29ybGQ=");
    }

    #[test]
    fn test_websocket_accept_key() {
        let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept = compute_accept_key(client_key);
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn test_client_frame_roundtrip() {
        let mask = [0x12, 0x34, 0x56, 0x78];
        let payload = b"Hello WebSocket";
        let frame = encode_client_frame(Opcode::Text, payload, mask);

        let mut cursor = std::io::Cursor::new(frame);
        let (opcode, decoded_payload, fin) = decode_frame(&mut cursor).expect("decode failed");

        assert_eq!(opcode, Opcode::Text);
        assert!(fin);
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn test_decode_rejects_rsv_bits() {
        // FIN + RSV1 + Text opcode.
        let mut cursor = std::io::Cursor::new(vec![0xC1u8, 0x00]);
        assert!(decode_frame(&mut cursor).is_err());
    }

    #[test]
    fn test_decode_rejects_nonminimal_length() {
        // 126 with value 10 (should have used short form).
        let mut cursor = std::io::Cursor::new(vec![0x82u8, 126, 0, 10]);
        assert!(decode_frame(&mut cursor).is_err());
    }

    #[test]
    fn test_decode_rejects_fragmented_control() {
        // Ping with FIN=0.
        let mut cursor = std::io::Cursor::new(vec![0x09u8, 0x00]);
        assert!(decode_frame(&mut cursor).is_err());
    }

    #[test]
    fn test_decode_rejects_invalid_utf8_text() {
        // Text frame with invalid UTF-8 0xFF.
        let frame = vec![0x81u8, 0x01, 0xFF];
        let mut cursor = std::io::Cursor::new(frame);
        assert!(decode_frame(&mut cursor).is_err());
    }

    #[test]
    fn test_random_nonce_and_mask_vary() {
        let a = random_bytes_16();
        let b = random_bytes_16();
        // Extremely unlikely to collide; ensures not a fixed constant.
        assert_ne!(a, b);
        let m1 = random_mask();
        let m2 = random_mask();
        // Allow rare collision but require function works; check not both zero.
        assert!(!(m1 == [0, 0, 0, 0] && m2 == [0, 0, 0, 0]));
    }
}
