//! WebSocket client connection worker loop and TLS handling.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use super::crypto::{base64_encode, compute_accept_key, random_bytes_16, random_mask};
use super::frame::{Opcode, decode_frame, encode_client_frame};
use super::manager::{WebSocketCommand, WebSocketEvent};
use crate::url::Url;

pub const MAX_WS_PAYLOAD: usize = 16 * 1024 * 1024;
const MAX_HANDSHAKE_BYTES: usize = 64 * 1024;

// ── Unified Read+Write trait for plain TCP and TLS streams ────────────────────

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

/// Background worker loop managing the TCP stream and RFC 6455 protocol states.
pub(crate) fn run_websocket_client<F>(
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
    // We use a Box<dyn ReadWrite> to unify the plain and TLS paths.
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
