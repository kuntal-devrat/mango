//! Bidirectional IPC endpoint communication channels.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::time::Duration;

use crate::framing::IpcFrame;
use crate::protocol::IpcMessage;

/// Communication error during IPC operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcError {
    ChannelClosed,
    Timeout,
    SerializationError(String),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::ChannelClosed => write!(f, "IPC channel closed by remote peer"),
            IpcError::Timeout => write!(f, "IPC operation timed out"),
            IpcError::SerializationError(msg) => write!(f, "IPC serialization error: {msg}"),
        }
    }
}

impl std::error::Error for IpcError {}

/// A single endpoint of a duplex IPC channel.
pub struct IpcEndpoint {
    sender: Sender<IpcFrame>,
    receiver: Receiver<IpcFrame>,
    connected: Arc<AtomicBool>,
    correlation_seq: Arc<AtomicU32>,
}

impl IpcEndpoint {
    /// Creates a connected pair of bidirectional IPC endpoints.
    pub fn create_pair() -> (IpcEndpoint, IpcEndpoint) {
        let (tx_a, rx_b) = mpsc::channel();
        let (tx_b, rx_a) = mpsc::channel();
        let connected = Arc::new(AtomicBool::new(true));
        let correlation_seq = Arc::new(AtomicU32::new(1));

        let endpoint_a = IpcEndpoint {
            sender: tx_a,
            receiver: rx_a,
            connected: connected.clone(),
            correlation_seq: correlation_seq.clone(),
        };

        let endpoint_b = IpcEndpoint {
            sender: tx_b,
            receiver: rx_b,
            connected,
            correlation_seq,
        };

        (endpoint_a, endpoint_b)
    }

    /// True if the peer connection is active.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    /// Sends a strongly-typed message across the channel.
    pub fn send(&self, msg: &IpcMessage) -> Result<u32, IpcError> {
        if !self.is_connected() {
            return Err(IpcError::ChannelClosed);
        }
        let cid = self.correlation_seq.fetch_add(1, Ordering::Relaxed);
        let frame = msg
            .to_frame(cid, false)
            .map_err(IpcError::SerializationError)?;
        self.sender.send(frame).map_err(|_| {
            self.connected.store(false, Ordering::Release);
            IpcError::ChannelClosed
        })?;
        Ok(cid)
    }

    /// Sends a response frame matching a given correlation ID.
    pub fn send_response(&self, correlation_id: u32, msg: &IpcMessage) -> Result<(), IpcError> {
        if !self.is_connected() {
            return Err(IpcError::ChannelClosed);
        }
        let frame = msg
            .to_frame(correlation_id, true)
            .map_err(IpcError::SerializationError)?;
        self.sender.send(frame).map_err(|_| {
            self.connected.store(false, Ordering::Release);
            IpcError::ChannelClosed
        })?;
        Ok(())
    }

    /// Attempts to receive a message without blocking.
    pub fn try_recv(&self) -> Result<Option<IpcMessage>, IpcError> {
        match self.receiver.try_recv() {
            Ok(frame) => {
                let msg = IpcMessage::from_frame(&frame).map_err(IpcError::SerializationError)?;
                Ok(Some(msg))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                self.connected.store(false, Ordering::Release);
                Err(IpcError::ChannelClosed)
            }
        }
    }

    /// Blocks waiting for an incoming message up to `timeout`.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<IpcMessage, IpcError> {
        match self.receiver.recv_timeout(timeout) {
            Ok(frame) => {
                let msg = IpcMessage::from_frame(&frame).map_err(IpcError::SerializationError)?;
                Ok(msg)
            }
            Err(RecvTimeoutError::Timeout) => Err(IpcError::Timeout),
            Err(RecvTimeoutError::Disconnected) => {
                self.connected.store(false, Ordering::Release);
                Err(IpcError::ChannelClosed)
            }
        }
    }

    /// Gracefully closes the connection endpoint.
    pub fn close(&self) {
        self.connected.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_endpoint_communication() {
        let (ep1, ep2) = IpcEndpoint::create_pair();

        let ping = IpcMessage::Ping { seq: 42 };
        ep1.send(&ping).expect("send ping");

        let msg = ep2
            .recv_timeout(Duration::from_millis(50))
            .expect("recv ping");
        assert_eq!(msg, ping);

        let pong = IpcMessage::Pong { seq: 42 };
        ep2.send(&pong).expect("send pong");

        let res = ep1
            .recv_timeout(Duration::from_millis(50))
            .expect("recv pong");
        assert_eq!(res, pong);
    }
}
