//! HTTP/2 binary framing layer (RFC 7540 §4 / RFC 9113 §4).

/// HTTP/2 Connection Preface sent by clients upon establishing connection.
pub const HTTP2_CLIENT_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

use super::session::StreamPriority;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Data,
    Headers,
    Priority,
    RstStream,
    Settings,
    PushPromise,
    Ping,
    GoAway,
    WindowUpdate,
    Continuation,
    Unknown(u8),
}

impl From<u8> for FrameType {
    fn from(val: u8) -> Self {
        match val {
            0x0 => FrameType::Data,
            0x1 => FrameType::Headers,
            0x2 => FrameType::Priority,
            0x3 => FrameType::RstStream,
            0x4 => FrameType::Settings,
            0x5 => FrameType::PushPromise,
            0x6 => FrameType::Ping,
            0x7 => FrameType::GoAway,
            0x8 => FrameType::WindowUpdate,
            0x9 => FrameType::Continuation,
            other => FrameType::Unknown(other),
        }
    }
}

impl From<FrameType> for u8 {
    fn from(val: FrameType) -> Self {
        match val {
            FrameType::Data => 0x0,
            FrameType::Headers => 0x1,
            FrameType::Priority => 0x2,
            FrameType::RstStream => 0x3,
            FrameType::Settings => 0x4,
            FrameType::PushPromise => 0x5,
            FrameType::Ping => 0x6,
            FrameType::GoAway => 0x7,
            FrameType::WindowUpdate => 0x8,
            FrameType::Continuation => 0x9,
            FrameType::Unknown(code) => code,
        }
    }
}

/// 9-octet binary frame header preceding all HTTP/2 frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    /// 24-bit payload length (max 16,777,215 octets).
    pub length: u32,
    /// 8-bit frame type.
    pub frame_type: FrameType,
    /// 8-bit frame flags (e.g. END_STREAM = 0x1, END_HEADERS = 0x4).
    pub flags: u8,
    /// 31-bit stream identifier (bit 32 is reserved).
    pub stream_id: u32,
}

impl FrameHeader {
    /// Serializes the frame header into standard 9 octets.
    pub fn serialize(&self) -> [u8; 9] {
        let mut buf = [0u8; 9];
        // 24-bit length
        buf[0] = ((self.length >> 16) & 0xFF) as u8;
        buf[1] = ((self.length >> 8) & 0xFF) as u8;
        buf[2] = (self.length & 0xFF) as u8;
        // 8-bit type
        buf[3] = self.frame_type.into();
        // 8-bit flags
        buf[4] = self.flags;
        // 31-bit stream ID (masking highest bit)
        let s = self.stream_id & 0x7FFF_FFFF;
        buf[5] = ((s >> 24) & 0xFF) as u8;
        buf[6] = ((s >> 16) & 0xFF) as u8;
        buf[7] = ((s >> 8) & 0xFF) as u8;
        buf[8] = (s & 0xFF) as u8;
        buf
    }

    /// Deserializes a frame header from a 9-octet byte slice.
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 9 {
            return None;
        }
        let length = ((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[2] as u32);
        let frame_type = FrameType::from(buf[3]);
        let flags = buf[4];
        let stream_id = (((buf[5] as u32) & 0x7F) << 24)
            | ((buf[6] as u32) << 16)
            | ((buf[7] as u32) << 8)
            | (buf[8] as u32);

        Some(Self {
            length,
            frame_type,
            flags,
            stream_id,
        })
    }
}

/// A complete HTTP/2 frame with parsed header and payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Http2Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

impl Http2Frame {
    /// Creates a new HTTP/2 frame.
    pub fn new(frame_type: FrameType, flags: u8, stream_id: u32, payload: Vec<u8>) -> Self {
        Self {
            header: FrameHeader {
                length: payload.len() as u32,
                frame_type,
                flags,
                stream_id,
            },
            payload,
        }
    }

    /// Creates a SETTINGS frame (stream 0).
    pub fn settings(params: &[(u16, u32)]) -> Self {
        let mut payload = Vec::with_capacity(params.len() * 6);
        for &(id, val) in params {
            payload.extend_from_slice(&id.to_be_bytes());
            payload.extend_from_slice(&val.to_be_bytes());
        }
        Self::new(FrameType::Settings, 0x0, 0, payload)
    }

    /// Creates a SETTINGS ACK frame.
    pub fn settings_ack() -> Self {
        Self::new(FrameType::Settings, 0x1, 0, Vec::new())
    }

    /// Creates a PING frame with 8-octet opaque data.
    pub fn ping(data: [u8; 8], ack: bool) -> Self {
        let flags = if ack { 0x1 } else { 0x0 };
        Self::new(FrameType::Ping, flags, 0, data.to_vec())
    }

    /// Creates a WINDOW_UPDATE frame.
    pub fn window_update(stream_id: u32, increment: u32) -> Self {
        let payload = (increment & 0x7FFF_FFFF).to_be_bytes().to_vec();
        Self::new(FrameType::WindowUpdate, 0x0, stream_id, payload)
    }

    /// Creates a PRIORITY frame (RFC 7540 §6.3 / RFC 9113).
    pub fn priority(stream_id: u32, priority: StreamPriority) -> Self {
        let mut payload = [0u8; 5];
        let dep = (priority.parent_stream_id & 0x7FFF_FFFF)
            | if priority.exclusive { 0x8000_0000 } else { 0 };
        payload[..4].copy_from_slice(&dep.to_be_bytes());
        payload[4] = priority.weight.saturating_sub(1);
        Self::new(FrameType::Priority, 0x0, stream_id, payload.to_vec())
    }

    /// Serializes the entire frame into a byte vector.
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(9 + self.payload.len());
        bytes.extend_from_slice(&self.header.serialize());
        bytes.extend_from_slice(&self.payload);
        bytes
    }
}
