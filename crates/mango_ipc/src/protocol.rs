//! Typed message definitions and serialization for cross-process communication.

use serde::{Deserialize, Serialize};

use crate::framing::{FLAG_RESPONSE, IpcFrame, IpcHeader};

/// The distinct roles of browser processes conforming to Chromium architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessType {
    /// Privileged browser coordinator owning UI, windows, profile, and navigation.
    Browser,
    /// Sandboxed Web content executor (V8 runtime + DOM + LayoutNG).
    Renderer,
    /// Network protocol engine (HTTP/1.1, H2, H3/QUIC, DNS, TLS).
    Network,
    /// Hardware tile rasterizer, WebGPU pipeline, and compositor.
    Gpu,
}

/// Core IPC message definitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IpcMessage {
    // Process Lifecycle
    Init {
        process_id: u32,
        process_type: ProcessType,
        site: String,
    },
    InitAck {
        process_id: u32,
    },
    Ping {
        seq: u64,
    },
    Pong {
        seq: u64,
    },
    Terminate,

    // Navigation & Site Isolation
    Navigate {
        target_url: String,
        history_id: u64,
    },
    DidCommitProvisionalLoad {
        url: String,
        title: String,
        http_status: u16,
    },
    DocumentLoaded {
        url: String,
    },
    RenderProcessCrashed {
        process_id: u32,
        exit_code: i32,
    },

    // Rendering & Compositor
    RenderFrame {
        frame_id: u64,
        width: u32,
        height: u32,
        shm_offset: usize,
        data_len: usize,
    },
    SwapBuffers {
        frame_id: u64,
    },
    InvalidateRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },

    // Input & User Events
    InputEvent {
        event_type: String,
        x: f32,
        y: f32,
        key_code: u32,
        modifiers: u32,
    },

    // Network & Resource Fetching
    FetchRequest {
        request_id: u64,
        url: String,
        method: String,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
    },
    FetchResponseHeader {
        request_id: u64,
        status: u16,
        headers: Vec<(String, String)>,
    },
    FetchDataChunk {
        request_id: u64,
        chunk: Vec<u8>,
    },
    FetchComplete {
        request_id: u64,
    },

    // Storage & Cookies
    StorageGetItem {
        origin: String,
        key: String,
    },
    StorageGetItemResult {
        value: Option<String>,
    },
    StorageSetItem {
        origin: String,
        key: String,
        value: String,
    },
    StorageRemoveItem {
        origin: String,
        key: String,
    },
}

impl IpcMessage {
    /// Computes the unique message type opcode.
    pub fn opcode(&self) -> u16 {
        match self {
            IpcMessage::Init { .. } => 1,
            IpcMessage::InitAck { .. } => 2,
            IpcMessage::Ping { .. } => 3,
            IpcMessage::Pong { .. } => 4,
            IpcMessage::Terminate => 5,

            IpcMessage::Navigate { .. } => 10,
            IpcMessage::DidCommitProvisionalLoad { .. } => 11,
            IpcMessage::DocumentLoaded { .. } => 12,
            IpcMessage::RenderProcessCrashed { .. } => 13,

            IpcMessage::RenderFrame { .. } => 20,
            IpcMessage::SwapBuffers { .. } => 21,
            IpcMessage::InvalidateRect { .. } => 22,

            IpcMessage::InputEvent { .. } => 30,

            IpcMessage::FetchRequest { .. } => 40,
            IpcMessage::FetchResponseHeader { .. } => 41,
            IpcMessage::FetchDataChunk { .. } => 42,
            IpcMessage::FetchComplete { .. } => 43,

            IpcMessage::StorageGetItem { .. } => 50,
            IpcMessage::StorageGetItemResult { .. } => 51,
            IpcMessage::StorageSetItem { .. } => 52,
            IpcMessage::StorageRemoveItem { .. } => 53,
        }
    }

    /// Encodes the message into an `IpcFrame`.
    pub fn to_frame(&self, correlation_id: u32, is_response: bool) -> Result<IpcFrame, String> {
        let payload = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        let mut flags = 0;
        if is_response {
            flags |= FLAG_RESPONSE;
        }
        Ok(IpcFrame {
            header: IpcHeader::new(self.opcode(), flags, correlation_id, payload.len() as u32),
            payload,
        })
    }

    /// Decodes an `IpcMessage` from an `IpcFrame`.
    pub fn from_frame(frame: &IpcFrame) -> Result<Self, String> {
        serde_json::from_slice(&frame.payload).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_message_to_from_frame() {
        let msg = IpcMessage::Navigate {
            target_url: "https://example.com/login".to_string(),
            history_id: 101,
        };

        let frame = msg.to_frame(42, false).expect("to_frame");
        assert_eq!(frame.header.msg_type, 10);
        assert_eq!(frame.header.correlation_id, 42);

        let decoded = IpcMessage::from_frame(&frame).expect("from_frame");
        assert_eq!(decoded, msg);
    }
}
