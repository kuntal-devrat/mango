//! Pure-Rust RFC 6455 WebSocket Client Implementation.

pub mod client;
pub mod crypto;
pub mod frame;
pub mod manager;

#[cfg(test)]
mod tests;

pub use client::MAX_WS_PAYLOAD;
pub use crypto::{base64_encode, compute_accept_key, random_bytes_16, random_mask, sha1};
pub use frame::{Opcode, decode_frame, encode_client_frame, encode_server_frame};
pub use manager::{WebSocketCommand, WebSocketConnection, WebSocketEvent, WebSocketManager};
