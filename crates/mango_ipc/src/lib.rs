//! # mango_ipc
//!
//! High-throughput zero-copy IPC and shared memory ring buffers for the Mango
//! browser multi-process architecture and site isolation model.
//!
//! Implements binary message framing, memory-mapped shared-memory ring buffers,
//! duplex channel endpoints, and Chromium-style process messaging protocols.

pub mod channel;
pub mod framing;
pub mod protocol;
pub mod shm;

pub use channel::{IpcEndpoint, IpcError};
pub use framing::{
    FLAG_RESPONSE, FLAG_SHM, FLAG_SYNC, IPC_HEADER_SIZE, IPC_MAGIC, IpcFrame, IpcHeader,
};
pub use protocol::{IpcMessage, ProcessType};
pub use shm::{ShmError, ShmRingBuffer};
