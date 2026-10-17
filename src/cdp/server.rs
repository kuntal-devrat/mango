//! Chrome DevTools Protocol HTTP & WebSocket Server.
//!
//! Provides the external control plane on `--remote-debugging-port=<port>`
//! to drive Mango with Puppeteer, Playwright, or DevTools frontends.
//!
//! Uses a dedicated worker thread for the CDP engine so that `JsRuntime`
//! (and the underlying V8 isolate) remains safely pinned to a single thread
//! while concurrent network connections are serviced asynchronously.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::json;

use super::session::CdpSession;
use super::target::CdpTarget;
use super::types::{JsonRpcRequest, JsonRpcResponse, TargetDescriptor, VersionInfo};

/// Commands dispatched from HTTP/WebSocket connection threads to the dedicated CDP worker thread.
pub enum CdpCmd {
    Http {
        method: String,
        path: String,
        responder: Sender<HttpResponsePayload>,
    },
    WsMessage {
        session_id: String,
        target_id: String,
        req: JsonRpcRequest,
        responder: Sender<JsonRpcResponse>,
    },
}

/// Outgoing HTTP response constructed by the CDP worker.
pub struct HttpResponsePayload {
    pub status_code: u16,
    pub status_text: String,
    pub content_type: String,
    pub body: String,
}

/// The embedded Chrome DevTools Protocol server instance.
pub struct CdpServer {
    pub cmd_tx: Sender<CdpCmd>,
    running: Arc<AtomicBool>,
    worker_handle: Option<JoinHandle<()>>,
    listener_handle: Option<JoinHandle<()>>,
    actual_port: u16,
}

impl CdpServer {
    /// Starts the CDP server on the specified TCP port (0 selects an OS-assigned ephemeral port).
    pub fn start(port: u16) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let actual_port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;

        let (cmd_tx, cmd_rx) = channel::<CdpCmd>();
        let running = Arc::new(AtomicBool::new(true));

        let w_running = running.clone();
        let worker_handle = thread::Builder::new()
            .name("mango-cdp-worker".to_string())
            .spawn(move || {
                let mut targets: HashMap<String, CdpTarget> = HashMap::new();
                let mut sessions: HashMap<String, CdpSession> = HashMap::new();
                let default_target = CdpTarget::new("page-1".to_string(), "about:blank");
                targets.insert("page-1".to_string(), default_target);
                let browser_id = "mango-browser-ctx-1".to_string();
                let user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36 Mango/0.2.3".to_string();

                while w_running.load(Ordering::Relaxed) {
                    match cmd_rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(CdpCmd::Http { method, path, responder }) => {
                            let resp = handle_http_cmd(
                                &method,
                                &path,
                                actual_port,
                                &browser_id,
                                &user_agent,
                                &mut targets,
                            );
                            let _ = responder.send(resp);
                        }
                        Ok(CdpCmd::WsMessage { session_id, target_id, req, responder }) => {
                            let session = sessions
                                .entry(session_id.clone())
                                .or_insert_with(|| CdpSession::new(session_id, target_id.clone()));
                            targets
                                .entry(target_id.clone())
                                .or_insert_with(|| CdpTarget::new(target_id, "about:blank"));
                            let resp = session.handle_request(&req, &mut targets);
                            let _ = responder.send(resp);
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;

        let l_running = running.clone();
        let l_tx = cmd_tx.clone();
        let listener_handle = thread::Builder::new()
            .name("mango-cdp-listener".to_string())
            .spawn(move || {
                log::info!("CDP server listening on http://127.0.0.1:{actual_port}");
                while l_running.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _addr)) => {
                            let c_tx = l_tx.clone();
                            let c_running = l_running.clone();
                            thread::spawn(move || {
                                handle_connection(stream, c_tx, c_running, actual_port);
                            });
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(e) => {
                            log::debug!("CDP accept error: {e}");
                            thread::sleep(Duration::from_millis(5));
                        }
                    }
                }
                log::info!("CDP server stopped on port {actual_port}");
            })?;

        Ok(Self {
            cmd_tx,
            running,
            worker_handle: Some(worker_handle),
            listener_handle: Some(listener_handle),
            actual_port,
        })
    }

    pub fn port(&self) -> u16 {
        self.actual_port
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.listener_handle.take() {
            let _ = h.join();
        }
        if let Some(h) = self.worker_handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for CdpServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn handle_http_cmd(
    method: &str,
    path: &str,
    port: u16,
    browser_id: &str,
    user_agent: &str,
    targets: &mut HashMap<String, CdpTarget>,
) -> HttpResponsePayload {
    match (method, path) {
        ("GET", "/json/version") => {
            let info = VersionInfo {
                browser: "Mango/0.2.3 Chrome/133.0.0.0".to_string(),
                protocol_version: "1.3".to_string(),
                user_agent: user_agent.to_string(),
                v8_version: "deno_core".to_string(),
                web_socket_debugger_url: format!("ws://127.0.0.1:{port}/devtools/browser/{browser_id}"),
            };
            HttpResponsePayload {
                status_code: 200,
                status_text: "OK".to_string(),
                content_type: "application/json; charset=UTF-8".to_string(),
                body: serde_json::to_string(&info).unwrap_or_default(),
            }
        }

        ("GET", "/json") | ("GET", "/json/list") => {
            let mut list = Vec::new();
            for (id, target) in targets.iter() {
                list.push(TargetDescriptor {
                    id: id.clone(),
                    target_type: "page".to_string(),
                    title: target.title.clone(),
                    url: target.url.clone(),
                    description: String::new(),
                    web_socket_debugger_url: format!("ws://127.0.0.1:{port}/devtools/page/{id}"),
                    devtools_frontend_url: format!(
                        "/devtools/inspector.html?ws=127.0.0.1:{port}/devtools/page/{id}"
                    ),
                });
            }
            HttpResponsePayload {
                status_code: 200,
                status_text: "OK".to_string(),
                content_type: "application/json; charset=UTF-8".to_string(),
                body: serde_json::to_string(&list).unwrap_or_default(),
            }
        }

        ("GET", p) | ("PUT", p) if p.starts_with("/json/new") => {
            let query_url = if let Some((_, q)) = p.split_once('?') {
                q.split('&')
                    .find_map(|pair| {
                        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                        if k == "url" { Some(v) } else { None }
                    })
                    .unwrap_or("about:blank")
            } else {
                "about:blank"
            };

            let new_id = format!("page-{}", targets.len() + 1);
            let mut target = CdpTarget::new(new_id.clone(), query_url);
            if query_url.starts_with("data:") || query_url.starts_with("file:") {
                target.load_url(query_url);
            }
            let title = target.title.clone();
            let url = target.url.clone();
            targets.insert(new_id.clone(), target);

            let descriptor = TargetDescriptor {
                id: new_id.clone(),
                target_type: "page".to_string(),
                title,
                url,
                description: String::new(),
                web_socket_debugger_url: format!("ws://127.0.0.1:{port}/devtools/page/{new_id}"),
                devtools_frontend_url: format!(
                    "/devtools/inspector.html?ws=127.0.0.1:{port}/devtools/page/{new_id}"
                ),
            };
            HttpResponsePayload {
                status_code: 200,
                status_text: "OK".to_string(),
                content_type: "application/json; charset=UTF-8".to_string(),
                body: serde_json::to_string(&descriptor).unwrap_or_default(),
            }
        }

        ("GET", p) if p.starts_with("/json/activate/") => HttpResponsePayload {
            status_code: 200,
            status_text: "OK".to_string(),
            content_type: "text/plain; charset=UTF-8".to_string(),
            body: "Target activated".to_string(),
        },

        ("GET", p) if p.starts_with("/json/close/") => {
            let tid = p.strip_prefix("/json/close/").unwrap_or("");
            targets.remove(tid);
            HttpResponsePayload {
                status_code: 200,
                status_text: "OK".to_string(),
                content_type: "text/plain; charset=UTF-8".to_string(),
                body: "Target is closing".to_string(),
            }
        }

        ("GET", "/json/protocol") => HttpResponsePayload {
            status_code: 200,
            status_text: "OK".to_string(),
            content_type: "application/json; charset=UTF-8".to_string(),
            body: r#"{"domains":[]}"#.to_string(),
        },

        _ => HttpResponsePayload {
            status_code: 404,
            status_text: "Not Found".to_string(),
            content_type: "text/plain; charset=UTF-8".to_string(),
            body: "404 Not Found".to_string(),
        },
    }
}

fn handle_connection(
    mut stream: TcpStream,
    cmd_tx: Sender<CdpCmd>,
    running: Arc<AtomicBool>,
    _port: u16,
) {
    let _ = stream.set_nonblocking(false);
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
        return;
    }

    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let path = parts[1];

    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    // ── WebSocket Upgrade Check ───────────────────────────────────────────────
    let is_upgrade = headers
        .get("upgrade")
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    if is_upgrade && (path.starts_with("/devtools/page/") || path.starts_with("/devtools/browser/")) {
        if let Some(ws_key) = headers.get("sec-websocket-key") {
            let accept = mango_net::websocket::compute_accept_key(ws_key);
            let resp = format!(
                "HTTP/1.1 101 Switching Protocols\r\n\
                 Upgrade: websocket\r\n\
                 Connection: Upgrade\r\n\
                 Sec-WebSocket-Accept: {accept}\r\n\r\n"
            );
            if stream.write_all(resp.as_bytes()).is_err() {
                return;
            }

            let target_id = if let Some(stripped) = path.strip_prefix("/devtools/page/") {
                stripped.to_string()
            } else {
                "page-1".to_string()
            };

            let session_id = format!("cdp-session-{}", target_id);

            // Run WebSocket loop
            handle_websocket_session(&mut stream, &session_id, &target_id, cmd_tx, running);
            return;
        }
    }

    // ── HTTP Discovery Endpoints ──────────────────────────────────────────────
    let (resp_tx, resp_rx) = channel::<HttpResponsePayload>();
    if cmd_tx
        .send(CdpCmd::Http {
            method: method.to_string(),
            path: path.to_string(),
            responder: resp_tx,
        })
        .is_ok()
    {
        if let Ok(resp) = resp_rx.recv_timeout(Duration::from_secs(5)) {
            send_http_response(&mut stream, resp.status_code, &resp.status_text, &resp.content_type, &resp.body);
        } else {
            send_http_response(&mut stream, 500, "Internal Server Error", "text/plain", "Worker timeout");
        }
    }
}

fn handle_websocket_session(
    stream: &mut TcpStream,
    session_id: &str,
    target_id: &str,
    cmd_tx: Sender<CdpCmd>,
    running: Arc<AtomicBool>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));

    while running.load(Ordering::Relaxed) {
        match mango_net::websocket::decode_frame(stream) {
            Ok((opcode, payload, _fin)) => match opcode {
                mango_net::websocket::Opcode::Text => {
                    let msg_str = match std::str::from_utf8(&payload) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                    let req: JsonRpcRequest = match serde_json::from_str(msg_str) {
                        Ok(r) => r,
                        Err(_) => {
                            let err_resp = json!({
                                "jsonrpc": "2.0",
                                "id": null,
                                "error": { "code": -32700, "message": "Parse error" }
                            });
                            let resp_bytes = serde_json::to_vec(&err_resp).unwrap_or_default();
                            let frame = mango_net::websocket::encode_server_frame(
                                mango_net::websocket::Opcode::Text,
                                &resp_bytes,
                            );
                            let _ = stream.write_all(&frame);
                            continue;
                        }
                    };

                    let (resp_tx, resp_rx) = channel::<JsonRpcResponse>();
                    if cmd_tx
                        .send(CdpCmd::WsMessage {
                            session_id: session_id.to_string(),
                            target_id: target_id.to_string(),
                            req,
                            responder: resp_tx,
                        })
                        .is_err()
                    {
                        break;
                    }

                    if let Ok(resp) = resp_rx.recv_timeout(Duration::from_secs(30)) {
                        let resp_bytes = serde_json::to_vec(&resp).unwrap_or_default();
                        let frame = mango_net::websocket::encode_server_frame(
                            mango_net::websocket::Opcode::Text,
                            &resp_bytes,
                        );
                        if stream.write_all(&frame).is_err() {
                            break;
                        }
                    }
                }
                mango_net::websocket::Opcode::Ping => {
                    let pong = mango_net::websocket::encode_server_frame(
                        mango_net::websocket::Opcode::Pong,
                        &payload,
                    );
                    let _ = stream.write_all(&pong);
                }
                mango_net::websocket::Opcode::Close => {
                    let close_frame = mango_net::websocket::encode_server_frame(
                        mango_net::websocket::Opcode::Close,
                        &[],
                    );
                    let _ = stream.write_all(&close_frame);
                    break;
                }
                _ => {}
            },
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // Timeout on nonblocking read, continue loop
                continue;
            }
            Err(_) => {
                // Connection closed or error
                break;
            }
        }
    }
}

fn send_http_response(
    stream: &mut TcpStream,
    code: u16,
    text: &str,
    content_type: &str,
    body: &str,
) {
    let resp = format!(
        "HTTP/1.1 {code} {text}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\r\n\
         {body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
}
