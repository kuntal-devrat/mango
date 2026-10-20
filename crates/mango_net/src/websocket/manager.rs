//! WebSocket connection handles and connection manager.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use super::client::run_websocket_client;

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
